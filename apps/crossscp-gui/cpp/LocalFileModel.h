// SPDX-License-Identifier: AGPL-3.0-or-later

#pragma once

#include <QAbstractListModel>
#include <QFileInfo>
#include <QVector>

class LocalFileModel : public QAbstractListModel {
  Q_OBJECT
  Q_PROPERTY(QString path READ path WRITE setPath NOTIFY pathChanged)
  Q_PROPERTY(QString error READ error NOTIFY errorChanged)

public:
  enum Roles {
    NameRole = Qt::UserRole + 1,
    PathRole,
    SizeRole,
    DirectoryRole,
    SymlinkRole,
    ModifiedRole,
    PermissionsRole
  };
  Q_ENUM(Roles)

  explicit LocalFileModel(QObject *parent = nullptr);

  int rowCount(const QModelIndex &parent = QModelIndex()) const override;
  QVariant data(const QModelIndex &index, int role = Qt::DisplayRole) const override;
  QHash<int, QByteArray> roleNames() const override;

  QString path() const;
  void setPath(const QString &path);
  QString error() const;

  Q_INVOKABLE void refresh();
  Q_INVOKABLE void openRow(int row);
  Q_INVOKABLE void goUp();
  Q_INVOKABLE bool createDirectory(const QString &name);
  Q_INVOKABLE bool deletePath(const QString &path);
  Q_INVOKABLE QString homePath() const;

signals:
  void pathChanged();
  void errorChanged();

private:
  void setError(const QString &error);

  QString m_path;
  QString m_error;
  QVector<QFileInfo> m_entries;
};
