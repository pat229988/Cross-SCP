// SPDX-License-Identifier: AGPL-3.0-or-later

#include "AppBackend.h"

#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QGuiApplication>
#include <QProcess>
#include <QStandardPaths>
#include <QStyleHints>
#include <QUrl>

AppBackend::AppBackend(QObject *parent) : QObject(parent) {
  const QString configRoot = QStandardPaths::writableLocation(
      QStandardPaths::AppConfigLocation);
  QDir().mkpath(configRoot);
  sessionsPath_ = QDir(configRoot).filePath("sessions.tsv");
  setStatus(QStringLiteral("Rust CLI bridge ready: %1").arg(resolveCliPath()));
}

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

bool AppBackend::saveSite(const QString &name, const QString &host, int port,
                          const QString &username, const QString &remotePath,
                          const QString &credentialRef) {
  if (name.trimmed().isEmpty() || host.trimmed().isEmpty()) {
    setStatus(QStringLiteral("Site name and host are required"));
    return false;
  }
  const CommandResult result = runCommand({QStringLiteral("session-save"), sessionsPath_,
                                           name.trimmed(), host.trimmed(),
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

  const QStringList knownKeyNames = {QStringLiteral("id_ed25519"), QStringLiteral("id_rsa"),
                                     QStringLiteral("id_ecdsa"), QStringLiteral("id_dsa"),
                                     QStringLiteral("identity")};
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

CommandResult AppBackend::runCommand(const QStringList &arguments, const QString &password,
                                     const QString &privateKeyPath,
                                     const QString &privateKeyPassphrase) {
  QProcess process;
  QProcessEnvironment environment = QProcessEnvironment::systemEnvironment();
  if (!password.isEmpty()) {
    environment.insert(QStringLiteral("CROSSSCP_SFTP_PASSWORD"), password);
  }
  if (!privateKeyPath.isEmpty()) {
    environment.insert(QStringLiteral("CROSSSCP_SFTP_KEY_PATH"), privateKeyPath);
  }
  if (!privateKeyPassphrase.isEmpty()) {
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
