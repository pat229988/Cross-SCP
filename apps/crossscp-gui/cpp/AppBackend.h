// SPDX-License-Identifier: AGPL-3.0-or-later

#pragma once

#include <QObject>
#include <QProcess>
#include <QString>
#include <QStringList>

struct CommandResult {
  int exitCode = -1;
  QString standardOutput;
  QString standardError;
};

class AppBackend : public QObject {
  Q_OBJECT
  Q_PROPERTY(QString status READ status NOTIFY statusChanged)
  Q_PROPERTY(QString sessionsPath READ sessionsPath CONSTANT)
  Q_PROPERTY(bool systemDarkMode READ systemDarkMode CONSTANT)

public:
  explicit AppBackend(QObject *parent = nullptr);
  ~AppBackend() override;

  QString status() const;
  QString sessionsPath() const;
  bool systemDarkMode() const;

  Q_INVOKABLE QStringList listSites();
  Q_INVOKABLE bool saveSite(const QString &protocol, const QString &name, const QString &host, int port,
                            const QString &username,
                            const QString &remotePath,
                            const QString &credentialRef);
  Q_INVOKABLE bool deleteSite(const QString &name);
  Q_INVOKABLE bool copyLocalFile(const QString &source,
                                 const QString &destination);
  Q_INVOKABLE QStringList listSshPrivateKeys() const;
  Q_INVOKABLE QString localPathFromUrl(const QString &url) const;
  Q_INVOKABLE QString cliPath() const;
  Q_INVOKABLE bool startSshTunnel(const QString &targetHost, int targetPort,
                                  const QString &localHost, int localPort,
                                  const QString &jumpUsername,
                                  const QString &jumpHost, int jumpPort,
                                  const QString &privateKeyPath,
                                  const QString &jumpPassword);
  Q_INVOKABLE bool startProxyJumpTunnel(const QString &targetHost, int targetPort,
                                        const QString &localHost, int localPort,
                                        const QString &finalUsername,
                                        const QString &finalHost, int finalPort,
                                        const QString &proxyJumpChain,
                                        const QString &privateKeyPath,
                                        const QString &jumpPassword);
  Q_INVOKABLE bool startManagedNestedTunnel(const QString &targetHost, int targetPort,
                                            const QString &localHost, int localPort,
                                            const QString &hopSpecs,
                                            const QString &finalUsername,
                                            const QString &finalHost, int finalPort,
                                            const QString &finalPrivateKeyPath,
                                            const QString &finalPassword);
  Q_INVOKABLE void stopSshTunnel();
  Q_INVOKABLE bool sshTunnelActive() const;

  CommandResult runCommand(const QStringList &arguments,
                           const QString &password = QString(),
                           const QString &privateKeyPath = QString(),
                           const QString &privateKeyPassphrase = QString());

signals:
  void statusChanged();

private:
  void setStatus(const QString &status);
  QString resolveCliPath() const;

  QString status_;
  QString sessionsPath_;
  QProcess *sshTunnelProcess_ = nullptr;
  QString sshAskPassPath_;
  QString sshConfigPath_;
};
