// SPDX-License-Identifier: AGPL-3.0-or-later

#pragma once

#include <QAbstractListModel>

#include "AppBackend.h"

class RemoteFileModel : public QAbstractListModel {
  Q_OBJECT
  Q_PROPERTY(QString path READ path WRITE setPath NOTIFY pathChanged)
  Q_PROPERTY(QString error READ error NOTIFY errorChanged)
  Q_PROPERTY(bool connected READ connected NOTIFY connectedChanged)

public:
  enum Roles { NameRole = Qt::UserRole + 1, PathRole, SizeRole, IsDirectoryRole };

  explicit RemoteFileModel(QObject *parent = nullptr);

  int rowCount(const QModelIndex &parent = QModelIndex()) const override;
  QVariant data(const QModelIndex &index, int role = Qt::DisplayRole) const override;
  QHash<int, QByteArray> roleNames() const override;

  QString path() const;
  void setPath(const QString &path);
  QString error() const;
  bool connected() const;

  Q_INVOKABLE void setBackend(AppBackend *backend);
  Q_INVOKABLE bool connectPassword(const QString &host, int port, const QString &username,
                                   const QString &password, const QString &remotePath);
  Q_INVOKABLE bool connectKey(const QString &host, int port, const QString &username,
                              const QString &privateKeyPath,
                              const QString &privateKeyPassphrase,
                              const QString &remotePath);
  Q_INVOKABLE void refresh();
  Q_INVOKABLE void openRow(int row);
  Q_INVOKABLE void goUp();
  Q_INVOKABLE void disconnect();
  Q_INVOKABLE bool uploadFile(const QString &localPath, const QString &remotePath);
  Q_INVOKABLE bool downloadFile(const QString &remotePath, const QString &localPath);
  Q_INVOKABLE bool createDirectory(const QString &name);
  Q_INVOKABLE bool deletePath(const QString &remotePath);

signals:
  void pathChanged();
  void errorChanged();
  void connectedChanged();

private:
  struct Entry {
    QString name;
    QString path;
    qlonglong size = 0;
    bool isDirectory = false;
  };

  void setError(const QString &error);
  QString joinRemotePath(const QString &basePath, const QString &name) const;

  QList<Entry> entries_;
  AppBackend *backend_ = nullptr;
  QString host_;
  int port_ = 22;
  QString username_;
  QString password_;
  QString privateKeyPath_;
  QString privateKeyPassphrase_;
  QString path_ = QStringLiteral("/");
  QString error_;
  bool connected_ = false;
};
