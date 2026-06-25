// SPDX-License-Identifier: AGPL-3.0-or-later

#pragma once

#include <QAbstractListModel>
#include <QProcess>

#include "AppBackend.h"

class TransferQueueModel : public QAbstractListModel {
  Q_OBJECT
  Q_PROPERTY(bool useOpenSshBackend READ useOpenSshBackend WRITE setUseOpenSshBackend NOTIFY useOpenSshBackendChanged)

public:
  enum Roles {
    DirectionRole = Qt::UserRole + 1,
    SourceRole,
    DestinationRole,
    StateRole,
    ProgressRole,
    BytesDoneRole,
    BytesTotalRole,
    ErrorRole,
    SpeedRole,
    SpeedTextRole
  };

  explicit TransferQueueModel(QObject *parent = nullptr);

  int rowCount(const QModelIndex &parent = QModelIndex()) const override;
  QVariant data(const QModelIndex &index, int role = Qt::DisplayRole) const override;
  QHash<int, QByteArray> roleNames() const override;
  bool useOpenSshBackend() const;
  void setUseOpenSshBackend(bool enabled);

  Q_INVOKABLE void setBackend(AppBackend *backend);
  Q_INVOKABLE bool enqueueLocalCopy(const QString &source, const QString &destination);
  Q_INVOKABLE bool enqueueSftpUpload(const QString &host, int port,
                                     const QString &username,
                                     const QString &password,
                                     const QString &privateKeyPath,
                                     const QString &privateKeyPassphrase,
                                     const QString &source,
                                     const QString &destination);
  Q_INVOKABLE bool enqueueSftpDownload(const QString &host, int port,
                                       const QString &username,
                                       const QString &password,
                                       const QString &privateKeyPath,
                                       const QString &privateKeyPassphrase,
                                       const QString &source,
                                       const QString &destination);
  Q_INVOKABLE bool enqueueRemoteUpload(const QString &protocol, const QString &host,
                                       int port, const QString &username,
                                       const QString &password,
                                       const QString &privateKeyPath,
                                       const QString &privateKeyPassphrase,
                                       const QString &source,
                                       const QString &destination);
  Q_INVOKABLE bool enqueueRemoteDownload(const QString &protocol, const QString &host,
                                         int port, const QString &username,
                                         const QString &password,
                                         const QString &privateKeyPath,
                                         const QString &privateKeyPassphrase,
                                         const QString &source,
                                         const QString &destination);
  Q_INVOKABLE void clearFinished();
  Q_INVOKABLE void clearAll();

signals:
  void useOpenSshBackendChanged();
  void transferCompleted(const QString &direction, const QString &source,
                         const QString &destination);
  void transferFailed(const QString &direction, const QString &source,
                      const QString &destination, const QString &error);

private:
  struct Job {
    QString direction;
    QString source;
    QString destination;
    QString state;
    int progress = 0;
    qlonglong bytesDone = 0;
    qlonglong bytesTotal = 0;
    qlonglong speedBytesPerSecond = 0;
    qint64 startedAtMs = 0;
    QString error;
    QString program;
    QStringList arguments;
    QString protocol;
    QString password;
    QString privateKeyPath;
    QString privateKeyPassphrase;
    QString askPassPath;
    bool usesOpenSsh = false;
  };

  bool enqueueSftpTransfer(const QString &direction, const QString &host, int port,
                           const QString &username, const QString &password,
                           const QString &privateKeyPath,
                           const QString &privateKeyPassphrase,
                           const QString &source, const QString &destination);
  bool enqueueRemoteTransfer(const QString &direction, const QString &protocol,
                              const QString &host, int port,
                              const QString &username, const QString &password,
                              const QString &privateKeyPath,
                              const QString &privateKeyPassphrase,
                              const QString &source, const QString &destination);
  QStringList openSshScpArguments(bool upload, const QString &host, int port,
                                  const QString &username,
                                  const QString &privateKeyPath,
                                  const QString &source,
                                  const QString &destination) const;
  QString openSshRemoteSpec(const QString &username, const QString &host,
                            const QString &path) const;
  bool prepareOpenSshAskPass(Job &job);
  void cleanupOpenSshAskPass(const Job &job) const;
  void startNextQueuedTransfer();
  void startCurrentProcess();
  void consumeProgressOutput();
  void processProgressLine(const QString &line);
  void finishCurrentProcess(int exitCode, QProcess::ExitStatus exitStatus);
  void failCurrentProcess(const QString &error);
  void updateRow(int row);
  void markRowFailed(int row, const QString &error);
  QString formatBytes(qlonglong bytes) const;
  QString formatSpeed(qlonglong bytesPerSecond) const;

  QList<Job> jobs_;
  AppBackend *backend_ = nullptr;
  QProcess *currentProcess_ = nullptr;
  int currentRow_ = -1;
  QByteArray progressBuffer_;
  QByteArray errorOutput_;
  QByteArray standardOutput_;
  bool useOpenSshBackend_ = false;
};
