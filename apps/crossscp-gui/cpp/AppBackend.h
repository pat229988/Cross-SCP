// SPDX-License-Identifier: AGPL-3.0-or-later

#pragma once

#include <QObject>
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

public:
  explicit AppBackend(QObject *parent = nullptr);

  QString status() const;
  QString sessionsPath() const;

  Q_INVOKABLE QStringList listSites();
  Q_INVOKABLE bool saveSite(const QString &name, const QString &host, int port,
                            const QString &username,
                            const QString &remotePath,
                            const QString &credentialRef);
  Q_INVOKABLE bool deleteSite(const QString &name);
  Q_INVOKABLE bool copyLocalFile(const QString &source,
                                 const QString &destination);
  Q_INVOKABLE QStringList listSshPrivateKeys() const;
  Q_INVOKABLE QString localPathFromUrl(const QString &url) const;
  Q_INVOKABLE QString cliPath() const;

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
};
