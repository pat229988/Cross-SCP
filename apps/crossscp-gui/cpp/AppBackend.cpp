// SPDX-License-Identifier: AGPL-3.0-or-later

#include "AppBackend.h"

#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QGuiApplication>
#include <QProcess>
#include <QProcessEnvironment>
#include <QStandardPaths>
#include <QStyleHints>
#include <QTextStream>
#include <QUrl>

AppBackend::AppBackend(QObject *parent) : QObject(parent) {
  const QString configRoot = QStandardPaths::writableLocation(
      QStandardPaths::AppConfigLocation);
  QDir().mkpath(configRoot);
  sessionsPath_ = QDir(configRoot).filePath("sessions.tsv");
  setStatus(QStringLiteral("Rust CLI bridge ready: %1").arg(resolveCliPath()));
}

AppBackend::~AppBackend() { stopSshTunnel(); }

QString AppBackend::status() const { return status_; }

QString AppBackend::sessionsPath() const { return sessionsPath_; }

bool AppBackend::systemDarkMode() const {
#if QT_VERSION >= QT_VERSION_CHECK(6, 5, 0)
  return QGuiApplication::styleHints()->colorScheme() == Qt::ColorScheme::Dark;
#else
  return false;
#endif
}

QString AppBackend::cliPath() const { return resolveCliPath(); }

QStringList AppBackend::listSites() {
  const CommandResult result = runCommand({QStringLiteral("session-list"), sessionsPath_});
  if (result.exitCode != 0) {
    setStatus(QStringLiteral("Session load failed: %1").arg(result.standardError.trimmed()));
    return {};
  }
  setStatus(QStringLiteral("Loaded saved sites from Rust session store"));
  return result.standardOutput.split('\n', Qt::SkipEmptyParts);
}

bool AppBackend::saveSite(const QString &protocol, const QString &name, const QString &host, int port,
                          const QString &username, const QString &remotePath,
                          const QString &credentialRef) {
  if (name.trimmed().isEmpty() || host.trimmed().isEmpty()) {
    setStatus(QStringLiteral("Site name and host are required"));
    return false;
  }
  const CommandResult result = runCommand({QStringLiteral("session-save"), sessionsPath_,
                                           protocol.trimmed().toLower(), name.trimmed(), host.trimmed(),
                                           QString::number(port), username.trimmed(),
                                           remotePath.trimmed(), credentialRef.trimmed()});
  if (result.exitCode != 0) {
    setStatus(QStringLiteral("Save failed: %1").arg(result.standardError.trimmed()));
    return false;
  }
  setStatus(QStringLiteral("Saved site '%1' through Rust session persistence").arg(name));
  return true;
}

bool AppBackend::deleteSite(const QString &name) {
  if (name.trimmed().isEmpty()) {
    setStatus(QStringLiteral("Select a site to delete"));
    return false;
  }
  const CommandResult result = runCommand({QStringLiteral("session-delete"), sessionsPath_, name});
  if (result.exitCode != 0) {
    setStatus(QStringLiteral("Delete failed: %1").arg(result.standardError.trimmed()));
    return false;
  }
  setStatus(QStringLiteral("Deleted site '%1' through Rust session persistence").arg(name));
  return true;
}

bool AppBackend::copyLocalFile(const QString &source, const QString &destination) {
  const CommandResult result = runCommand({QStringLiteral("local-copy"), source, destination,
                                           QStringLiteral("always")});
  if (result.exitCode != 0) {
    setStatus(QStringLiteral("Transfer failed: %1").arg(result.standardError.trimmed()));
    return false;
  }
  setStatus(QStringLiteral("Transfer completed: %1").arg(result.standardOutput.trimmed()));
  return true;
}

QStringList AppBackend::listSshPrivateKeys() const {
  const QString home = QStandardPaths::writableLocation(QStandardPaths::HomeLocation);
  const QDir sshDir(QDir(home).filePath(QStringLiteral(".ssh")));
  if (!sshDir.exists()) {
    return {};
  }

  const QStringList knownKeyNames = {
      QStringLiteral("id_ed25519"),     QStringLiteral("id_rsa"),
      QStringLiteral("id_ecdsa"),       QStringLiteral("id_ecdsa_nistp256"),
      QStringLiteral("id_ecdsa_nistp384"), QStringLiteral("id_ecdsa_nistp521"),
      QStringLiteral("id_dsa"),         QStringLiteral("identity"),
      QStringLiteral("id_ed25519_sk"),  QStringLiteral("id_ecdsa_sk")};
  QStringList keys;
  const QFileInfoList entries = sshDir.entryInfoList(QDir::Files | QDir::Readable | QDir::Hidden,
                                                     QDir::Name);
  for (const QFileInfo &entry : entries) {
    const QString name = entry.fileName();
    if (name.endsWith(QStringLiteral(".pub")) || name == QStringLiteral("known_hosts") ||
        name.startsWith(QStringLiteral("known_hosts.")) || name == QStringLiteral("config") ||
        name == QStringLiteral("authorized_keys")) {
      continue;
    }

    QFile file(entry.absoluteFilePath());
    const bool looksLikePrivateKey = file.open(QIODevice::ReadOnly) &&
                                     QString::fromUtf8(file.read(256))
                                         .contains(QStringLiteral("PRIVATE KEY"));
    if (knownKeyNames.contains(name) || name.startsWith(QStringLiteral("id_")) ||
        looksLikePrivateKey) {
      keys.append(entry.absoluteFilePath());
    }
  }
  keys.removeDuplicates();
  keys.sort(Qt::CaseInsensitive);
  return keys;
}

QString AppBackend::localPathFromUrl(const QString &url) const {
  const QUrl parsed(url);
  if (parsed.isLocalFile()) {
    return parsed.toLocalFile();
  }
  return url;
}

bool AppBackend::startSshTunnel(const QString &targetHost, int targetPort,
                                const QString &localHost, int localPort,
                                const QString &jumpUsername,
                                const QString &jumpHost, int jumpPort,
                                const QString &privateKeyPath,
                                const QString &jumpPassword) {
  if (targetHost.trimmed().isEmpty() || jumpHost.trimmed().isEmpty()) {
    setStatus(QStringLiteral("SSH tunnel requires target host and jump host"));
    return false;
  }
  if (targetPort <= 0 || localPort <= 0 || jumpPort <= 0) {
    setStatus(QStringLiteral("SSH tunnel ports must be valid"));
    return false;
  }

  stopSshTunnel();

  const QString bindHost = localHost.trimmed().isEmpty() ? QStringLiteral("127.0.0.1")
                                                        : localHost.trimmed();
  const QString destination = QStringLiteral("%1:%2:%3")
                                  .arg(bindHost)
                                  .arg(localPort)
                                  .arg(QStringLiteral("%1:%2")
                                           .arg(targetHost.trimmed())
                                           .arg(targetPort));
  const QString jumpDestination = jumpUsername.trimmed().isEmpty()
                                      ? jumpHost.trimmed()
                                      : QStringLiteral("%1@%2")
                                            .arg(jumpUsername.trimmed(), jumpHost.trimmed());

  QStringList arguments = {QStringLiteral("-N"),
                           QStringLiteral("-L"),
                           destination,
                           QStringLiteral("-p"),
                           QString::number(jumpPort),
                           QStringLiteral("-o"),
                           QStringLiteral("ExitOnForwardFailure=yes"),
                           QStringLiteral("-o"),
                           QStringLiteral("ServerAliveInterval=30"),
                           QStringLiteral("-o"),
                           QStringLiteral("ServerAliveCountMax=3"),
                           QStringLiteral("-o"),
                           jumpPassword.isEmpty() ? QStringLiteral("BatchMode=yes")
                                                  : QStringLiteral("BatchMode=no"),
                           QStringLiteral("-o"),
                           QStringLiteral("StrictHostKeyChecking=accept-new")};
  if (!privateKeyPath.trimmed().isEmpty()) {
    arguments.append(QStringLiteral("-i"));
    arguments.append(privateKeyPath.trimmed());
  }
  arguments.append(jumpDestination);

  sshTunnelProcess_ = new QProcess(this);
  sshTunnelProcess_->setProgram(QStringLiteral("ssh"));
  sshTunnelProcess_->setArguments(arguments);
  sshTunnelProcess_->setProcessChannelMode(QProcess::SeparateChannels);
  QProcessEnvironment environment = QProcessEnvironment::systemEnvironment();
  if (!jumpPassword.isEmpty()) {
    const QString tempRoot = QStandardPaths::writableLocation(QStandardPaths::TempLocation);
    sshAskPassPath_ = QDir(tempRoot).filePath(
        QStringLiteral("crossscp-ssh-askpass-%1.sh").arg(QCoreApplication::applicationPid()));
    QFile askpass(sshAskPassPath_);
    if (!askpass.open(QIODevice::WriteOnly | QIODevice::Truncate | QIODevice::Text)) {
      setStatus(QStringLiteral("Unable to create temporary SSH askpass helper"));
      sshTunnelProcess_->deleteLater();
      sshTunnelProcess_ = nullptr;
      return false;
    }
    QTextStream stream(&askpass);
    stream << "#!/bin/sh\n";
    stream << "printf '%s\\n' \"$CROSSSCP_JUMP_PASSWORD\"\n";
    askpass.close();
    QFile::setPermissions(sshAskPassPath_, QFileDevice::ReadOwner |
                                               QFileDevice::WriteOwner |
                                               QFileDevice::ExeOwner);
    environment.insert(QStringLiteral("SSH_ASKPASS"), sshAskPassPath_);
    environment.insert(QStringLiteral("SSH_ASKPASS_REQUIRE"), QStringLiteral("force"));
    environment.insert(QStringLiteral("DISPLAY"), environment.value(QStringLiteral("DISPLAY"), QStringLiteral("crossscp")));
    environment.insert(QStringLiteral("CROSSSCP_JUMP_PASSWORD"), jumpPassword);
  }
  sshTunnelProcess_->setProcessEnvironment(environment);
  sshTunnelProcess_->start();

  if (!sshTunnelProcess_->waitForStarted(5000)) {
    setStatus(QStringLiteral("Unable to start ssh for jump-host tunnel"));
    if (!sshAskPassPath_.isEmpty()) {
      QFile::remove(sshAskPassPath_);
      sshAskPassPath_.clear();
    }
    sshTunnelProcess_->deleteLater();
    sshTunnelProcess_ = nullptr;
    return false;
  }

  if (sshTunnelProcess_->waitForFinished(1200)) {
    const QString error = QString::fromUtf8(sshTunnelProcess_->readAllStandardError()).trimmed();
    setStatus(QStringLiteral("SSH tunnel failed: %1").arg(error.isEmpty() ? QStringLiteral("ssh exited") : error));
    if (!sshAskPassPath_.isEmpty()) {
      QFile::remove(sshAskPassPath_);
      sshAskPassPath_.clear();
    }
    sshTunnelProcess_->deleteLater();
    sshTunnelProcess_ = nullptr;
    return false;
  }

  setStatus(QStringLiteral("SSH tunnel active: %1:%2 → %3:%4 via %5")
                .arg(bindHost)
                .arg(localPort)
                .arg(targetHost.trimmed())
                .arg(targetPort)
                .arg(jumpHost.trimmed()));
  return true;
}

bool AppBackend::startProxyJumpTunnel(const QString &targetHost, int targetPort,
                                      const QString &localHost, int localPort,
                                      const QString &finalUsername,
                                      const QString &finalHost, int finalPort,
                                      const QString &proxyJumpChain,
                                      const QString &privateKeyPath,
                                      const QString &jumpPassword) {
  if (targetHost.trimmed().isEmpty() || finalHost.trimmed().isEmpty() ||
      proxyJumpChain.trimmed().isEmpty()) {
    setStatus(QStringLiteral("Nested SSH tunnel requires target host, final SSH host, and at least one jump hop"));
    return false;
  }
  if (targetPort <= 0 || localPort <= 0 || finalPort <= 0) {
    setStatus(QStringLiteral("Nested SSH tunnel ports must be valid"));
    return false;
  }

  stopSshTunnel();

  const QString bindHost = localHost.trimmed().isEmpty() ? QStringLiteral("127.0.0.1")
                                                        : localHost.trimmed();
  const QString forward = QStringLiteral("%1:%2:%3:%4")
                              .arg(bindHost)
                              .arg(localPort)
                              .arg(targetHost.trimmed())
                              .arg(targetPort);
  const QString finalDestination = finalUsername.trimmed().isEmpty()
                                       ? finalHost.trimmed()
                                       : QStringLiteral("%1@%2")
                                             .arg(finalUsername.trimmed(), finalHost.trimmed());

  QStringList arguments = {QStringLiteral("-N"),
                           QStringLiteral("-L"),
                           forward,
                           QStringLiteral("-J"),
                           proxyJumpChain.trimmed(),
                           QStringLiteral("-p"),
                           QString::number(finalPort),
                           QStringLiteral("-o"),
                           QStringLiteral("ExitOnForwardFailure=yes"),
                           QStringLiteral("-o"),
                           QStringLiteral("ServerAliveInterval=30"),
                           QStringLiteral("-o"),
                           QStringLiteral("ServerAliveCountMax=3"),
                           QStringLiteral("-o"),
                           jumpPassword.isEmpty() ? QStringLiteral("BatchMode=yes")
                                                  : QStringLiteral("BatchMode=no"),
                           QStringLiteral("-o"),
                           QStringLiteral("StrictHostKeyChecking=accept-new")};
  if (!privateKeyPath.trimmed().isEmpty()) {
    arguments.append(QStringLiteral("-i"));
    arguments.append(privateKeyPath.trimmed());
  }
  arguments.append(finalDestination);

  sshTunnelProcess_ = new QProcess(this);
  sshTunnelProcess_->setProgram(QStringLiteral("ssh"));
  sshTunnelProcess_->setArguments(arguments);
  sshTunnelProcess_->setProcessChannelMode(QProcess::SeparateChannels);
  QProcessEnvironment environment = QProcessEnvironment::systemEnvironment();
  if (!jumpPassword.isEmpty()) {
    const QString tempRoot = QStandardPaths::writableLocation(QStandardPaths::TempLocation);
    sshAskPassPath_ = QDir(tempRoot).filePath(
        QStringLiteral("crossscp-ssh-askpass-%1.sh").arg(QCoreApplication::applicationPid()));
    QFile askpass(sshAskPassPath_);
    if (!askpass.open(QIODevice::WriteOnly | QIODevice::Truncate | QIODevice::Text)) {
      setStatus(QStringLiteral("Unable to create temporary SSH askpass helper"));
      sshTunnelProcess_->deleteLater();
      sshTunnelProcess_ = nullptr;
      return false;
    }
    QTextStream stream(&askpass);
    stream << "#!/bin/sh\n";
    stream << "printf '%s\\n' \"$CROSSSCP_JUMP_PASSWORD\"\n";
    askpass.close();
    QFile::setPermissions(sshAskPassPath_, QFileDevice::ReadOwner |
                                               QFileDevice::WriteOwner |
                                               QFileDevice::ExeOwner);
    environment.insert(QStringLiteral("SSH_ASKPASS"), sshAskPassPath_);
    environment.insert(QStringLiteral("SSH_ASKPASS_REQUIRE"), QStringLiteral("force"));
    environment.insert(QStringLiteral("DISPLAY"), environment.value(QStringLiteral("DISPLAY"), QStringLiteral("crossscp")));
    environment.insert(QStringLiteral("CROSSSCP_JUMP_PASSWORD"), jumpPassword);
  }
  sshTunnelProcess_->setProcessEnvironment(environment);
  sshTunnelProcess_->start();

  if (!sshTunnelProcess_->waitForStarted(5000)) {
    setStatus(QStringLiteral("Unable to start ssh for nested jump tunnel"));
    if (!sshAskPassPath_.isEmpty()) {
      QFile::remove(sshAskPassPath_);
      sshAskPassPath_.clear();
    }
    sshTunnelProcess_->deleteLater();
    sshTunnelProcess_ = nullptr;
    return false;
  }

  if (sshTunnelProcess_->waitForFinished(1200)) {
    const QString error = QString::fromUtf8(sshTunnelProcess_->readAllStandardError()).trimmed();
    setStatus(QStringLiteral("Nested SSH tunnel failed: %1").arg(error.isEmpty() ? QStringLiteral("ssh exited") : error));
    if (!sshAskPassPath_.isEmpty()) {
      QFile::remove(sshAskPassPath_);
      sshAskPassPath_.clear();
    }
    sshTunnelProcess_->deleteLater();
    sshTunnelProcess_ = nullptr;
    return false;
  }

  setStatus(QStringLiteral("Nested SSH tunnel active: %1:%2 → %3:%4 via %5 → %6")
                .arg(bindHost)
                .arg(localPort)
                .arg(targetHost.trimmed())
                .arg(targetPort)
                .arg(proxyJumpChain.trimmed(), finalHost.trimmed()));
  return true;
}

namespace {
struct HopSpec {
  QString username;
  QString host;
  int port = 22;
  QString keyPath;
  QString password;
};

QString shellSingleQuoted(const QString &value) {
  QString escaped = value;
  escaped.replace('\'', QStringLiteral("'\\''"));
  return QStringLiteral("'") + escaped + QStringLiteral("'");
}
}

bool AppBackend::startManagedNestedTunnel(const QString &targetHost, int targetPort,
                                          const QString &localHost, int localPort,
                                          const QString &hopSpecs,
                                          const QString &finalUsername,
                                          const QString &finalHost, int finalPort,
                                          const QString &finalPrivateKeyPath,
                                          const QString &finalPassword) {
  if (targetHost.trimmed().isEmpty() || finalHost.trimmed().isEmpty() ||
      hopSpecs.trimmed().isEmpty()) {
    setStatus(QStringLiteral("Nested tunnel requires target host, final SSH host, and at least one hop"));
    return false;
  }

  QList<HopSpec> hops;
  const QStringList lines = hopSpecs.split('\n', Qt::SkipEmptyParts);
  for (const QString &line : lines) {
    const QStringList fields = line.split('\t');
    if (fields.size() < 5 || fields[1].trimmed().isEmpty()) {
      continue;
    }
    HopSpec hop;
    hop.username = fields[0].trimmed();
    hop.host = fields[1].trimmed();
    hop.port = fields[2].toInt() > 0 ? fields[2].toInt() : 22;
    hop.keyPath = fields[3].trimmed();
    hop.password = fields[4];
    hops.append(hop);
  }
  if (hops.isEmpty()) {
    setStatus(QStringLiteral("Add at least one valid SSH hop"));
    return false;
  }

  stopSshTunnel();

  const QString tempRoot = QStandardPaths::writableLocation(QStandardPaths::TempLocation);
  sshConfigPath_ = QDir(tempRoot).filePath(
      QStringLiteral("crossscp-ssh-config-%1").arg(QCoreApplication::applicationPid()));
  QFile config(sshConfigPath_);
  if (!config.open(QIODevice::WriteOnly | QIODevice::Truncate | QIODevice::Text)) {
    setStatus(QStringLiteral("Unable to create temporary SSH config"));
    return false;
  }
  QTextStream configStream(&config);
  for (int i = 0; i < hops.size(); ++i) {
    const HopSpec &hop = hops.at(i);
    configStream << "Host crossscp-hop-" << i << "\n";
    configStream << "  HostName " << hop.host << "\n";
    if (!hop.username.isEmpty()) configStream << "  User " << hop.username << "\n";
    configStream << "  Port " << hop.port << "\n";
    if (!hop.keyPath.isEmpty()) configStream << "  IdentityFile " << hop.keyPath << "\n";
    configStream << "  StrictHostKeyChecking accept-new\n";
    configStream << "  ServerAliveInterval 30\n";
  }
  configStream << "Host crossscp-final\n";
  configStream << "  HostName " << finalHost.trimmed() << "\n";
  if (!finalUsername.trimmed().isEmpty()) configStream << "  User " << finalUsername.trimmed() << "\n";
  configStream << "  Port " << finalPort << "\n";
  if (!finalPrivateKeyPath.trimmed().isEmpty()) configStream << "  IdentityFile " << finalPrivateKeyPath.trimmed() << "\n";
  QStringList aliases;
  for (int i = 0; i < hops.size(); ++i) aliases.append(QStringLiteral("crossscp-hop-%1").arg(i));
  configStream << "  ProxyJump " << aliases.join(',') << "\n";
  configStream << "  StrictHostKeyChecking accept-new\n";
  config.close();
  QFile::setPermissions(sshConfigPath_, QFileDevice::ReadOwner | QFileDevice::WriteOwner);

  sshAskPassPath_ = QDir(tempRoot).filePath(
      QStringLiteral("crossscp-ssh-askpass-%1.sh").arg(QCoreApplication::applicationPid()));
  QFile askpass(sshAskPassPath_);
  if (!askpass.open(QIODevice::WriteOnly | QIODevice::Truncate | QIODevice::Text)) {
    setStatus(QStringLiteral("Unable to create temporary SSH askpass helper"));
    QFile::remove(sshConfigPath_);
    sshConfigPath_.clear();
    return false;
  }
  QTextStream ask(&askpass);
  ask << "#!/bin/sh\n";
  ask << "case \"$1\" in\n";
  for (int i = 0; i < hops.size(); ++i) {
    if (!hops.at(i).password.isEmpty()) {
      ask << "*" << hops.at(i).host << "*) printf '%s\\n' " << shellSingleQuoted(hops.at(i).password) << "; exit 0 ;;\n";
    }
  }
  if (!finalPassword.isEmpty()) {
    ask << "*" << finalHost.trimmed() << "*) printf '%s\\n' " << shellSingleQuoted(finalPassword) << "; exit 0 ;;\n";
  }
  ask << "*) printf '%s\\n' \"\" ;;\n";
  ask << "esac\n";
  askpass.close();
  QFile::setPermissions(sshAskPassPath_, QFileDevice::ReadOwner | QFileDevice::WriteOwner |
                                             QFileDevice::ExeOwner);

  const QString bindHost = localHost.trimmed().isEmpty() ? QStringLiteral("127.0.0.1")
                                                        : localHost.trimmed();
  const QString forward = QStringLiteral("%1:%2:%3:%4")
                              .arg(bindHost)
                              .arg(localPort)
                              .arg(targetHost.trimmed())
                              .arg(targetPort);
  QStringList arguments = {QStringLiteral("-F"), sshConfigPath_, QStringLiteral("-N"),
                           QStringLiteral("-L"), forward, QStringLiteral("-o"),
                           QStringLiteral("ExitOnForwardFailure=yes"), QStringLiteral("-o"),
                           QStringLiteral("BatchMode=no"), QStringLiteral("crossscp-final")};

  sshTunnelProcess_ = new QProcess(this);
  sshTunnelProcess_->setProgram(QStringLiteral("ssh"));
  sshTunnelProcess_->setArguments(arguments);
  sshTunnelProcess_->setProcessChannelMode(QProcess::SeparateChannels);
  QProcessEnvironment environment = QProcessEnvironment::systemEnvironment();
  environment.insert(QStringLiteral("SSH_ASKPASS"), sshAskPassPath_);
  environment.insert(QStringLiteral("SSH_ASKPASS_REQUIRE"), QStringLiteral("force"));
  environment.insert(QStringLiteral("DISPLAY"), environment.value(QStringLiteral("DISPLAY"), QStringLiteral("crossscp")));
  sshTunnelProcess_->setProcessEnvironment(environment);
  sshTunnelProcess_->start();
  if (!sshTunnelProcess_->waitForStarted(5000)) {
    const QString message = QStringLiteral("Unable to start ssh for managed nested tunnel");
    stopSshTunnel();
    setStatus(message);
    return false;
  }
  if (sshTunnelProcess_->waitForFinished(1200)) {
    const QString error = QString::fromUtf8(sshTunnelProcess_->readAllStandardError()).trimmed();
    const QString message = QStringLiteral("Managed nested tunnel failed: %1").arg(error.isEmpty() ? QStringLiteral("ssh exited") : error);
    stopSshTunnel();
    setStatus(message);
    return false;
  }
  setStatus(QStringLiteral("Managed nested tunnel active: %1:%2 → %3:%4 through %5 hops")
                .arg(bindHost)
                .arg(localPort)
                .arg(targetHost.trimmed())
                .arg(targetPort)
                .arg(hops.size()));
  return true;
}

void AppBackend::stopSshTunnel() {
  if (sshTunnelProcess_ == nullptr) {
    return;
  }
  if (sshTunnelProcess_->state() != QProcess::NotRunning) {
    sshTunnelProcess_->terminate();
    if (!sshTunnelProcess_->waitForFinished(2000)) {
      sshTunnelProcess_->kill();
      sshTunnelProcess_->waitForFinished(1000);
    }
  }
  sshTunnelProcess_->deleteLater();
  sshTunnelProcess_ = nullptr;
  if (!sshAskPassPath_.isEmpty()) {
    QFile::remove(sshAskPassPath_);
    sshAskPassPath_.clear();
  }
  if (!sshConfigPath_.isEmpty()) {
    QFile::remove(sshConfigPath_);
    sshConfigPath_.clear();
  }
  setStatus(QStringLiteral("SSH tunnel stopped"));
}

bool AppBackend::sshTunnelActive() const {
  return sshTunnelProcess_ != nullptr && sshTunnelProcess_->state() != QProcess::NotRunning;
}

CommandResult AppBackend::runCommand(const QStringList &arguments, const QString &password,
                                     const QString &privateKeyPath,
                                     const QString &privateKeyPassphrase) {
  QProcess process;
  QProcessEnvironment environment = QProcessEnvironment::systemEnvironment();
  if (!password.isEmpty()) {
    environment.insert(QStringLiteral("CROSSSCP_REMOTE_PASSWORD"), password);
    environment.insert(QStringLiteral("CROSSSCP_SFTP_PASSWORD"), password);
  }
  if (!privateKeyPath.isEmpty()) {
    environment.insert(QStringLiteral("CROSSSCP_REMOTE_PRIVATE_KEY_PATH"), privateKeyPath);
    environment.insert(QStringLiteral("CROSSSCP_SFTP_KEY_PATH"), privateKeyPath);
  }
  if (!privateKeyPassphrase.isEmpty()) {
    environment.insert(QStringLiteral("CROSSSCP_REMOTE_PRIVATE_KEY_PASSPHRASE"), privateKeyPassphrase);
    environment.insert(QStringLiteral("CROSSSCP_SFTP_KEY_PASSPHRASE"), privateKeyPassphrase);
  }
  process.setProcessEnvironment(environment);
  process.start(resolveCliPath(), arguments);
  if (!process.waitForStarted(5000)) {
    return {-1, QString(), QStringLiteral("unable to start crossscp CLI")};
  }
  process.closeWriteChannel();
  if (!process.waitForFinished(120000)) {
    process.kill();
    return {-1, QString(), QStringLiteral("crossscp CLI timed out")};
  }
  return {process.exitCode(), QString::fromUtf8(process.readAllStandardOutput()),
          QString::fromUtf8(process.readAllStandardError())};
}

void AppBackend::setStatus(const QString &status) {
  if (status_ == status) {
    return;
  }
  status_ = status;
  emit statusChanged();
}

QString AppBackend::resolveCliPath() const {
  const QDir appDir(QCoreApplication::applicationDirPath());
#ifdef Q_OS_WIN
  const QStringList candidates = {
      appDir.filePath(QStringLiteral("crossscp-cli.exe")),
      appDir.filePath(QStringLiteral("../Resources/crossscp-cli.exe")),
      appDir.filePath(QStringLiteral("../../../../target/debug/crossscp.exe")),
      appDir.filePath(QStringLiteral("../../../../target/release/crossscp.exe")),
      QStringLiteral("crossscp.exe"),
      QStringLiteral("crossscp")};
#else
  const QStringList candidates = {
      appDir.filePath(QStringLiteral("crossscp-cli")),
      appDir.filePath(QStringLiteral("../Resources/crossscp-cli")),
      appDir.filePath(QStringLiteral("../../../../target/debug/crossscp")),
      appDir.filePath(QStringLiteral("../../../../target/release/crossscp")),
      QStringLiteral("crossscp")};
#endif
  for (const QString &candidate : candidates) {
    if (candidate == QStringLiteral("crossscp") ||
        candidate == QStringLiteral("crossscp.exe") || QFileInfo::exists(candidate)) {
      return candidate;
    }
  }
  return QStringLiteral("crossscp");
}
