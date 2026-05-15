// SPDX-License-Identifier: AGPL-3.0-or-later

#include "LocalFileModel.h"

#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QFileInfoList>
#include <QRegularExpression>
#include <QStandardPaths>

LocalFileModel::LocalFileModel(QObject *parent) : QAbstractListModel(parent) { setPath(homePath()); }

int LocalFileModel::rowCount(const QModelIndex &parent) const {
  return parent.isValid() ? 0 : static_cast<int>(m_entries.size());
}

QVariant LocalFileModel::data(const QModelIndex &index, int role) const {
  if (!index.isValid() || index.row() < 0 || index.row() >= m_entries.size()) {
    return {};
  }

  const QFileInfo &entry = m_entries.at(index.row());
  switch (role) {
  case NameRole:
    return entry.fileName();
  case PathRole:
    return entry.absoluteFilePath();
  case SizeRole:
    return entry.isFile() ? entry.size() : 0;
  case DirectoryRole:
    return entry.isDir();
  case SymlinkRole:
    return entry.isSymLink();
  case ModifiedRole:
    return entry.lastModified().toString(Qt::ISODate);
  case PermissionsRole:
    return QString::number(static_cast<uint>(entry.permissions()), 16);
  default:
    return {};
  }
}

QHash<int, QByteArray> LocalFileModel::roleNames() const {
  return {{NameRole, "name"},         {PathRole, "path"},       {SizeRole, "size"},
          {DirectoryRole, "isDirectory"}, {SymlinkRole, "isSymlink"}, {ModifiedRole, "modified"},
          {PermissionsRole, "permissions"}};
}

QString LocalFileModel::path() const { return m_path; }

void LocalFileModel::setPath(const QString &path) {
  const QString cleanPath = QDir::cleanPath(path.isEmpty() ? homePath() : path);
  if (m_path == cleanPath) {
    refresh();
    return;
  }
  m_path = cleanPath;
  emit pathChanged();
  refresh();
}

QString LocalFileModel::error() const { return m_error; }

void LocalFileModel::refresh() {
  QDir dir(m_path);
  if (!dir.exists()) {
    beginResetModel();
    m_entries.clear();
    endResetModel();
    setError(tr("Directory does not exist: %1").arg(m_path));
    return;
  }

  QFileInfoList entries = dir.entryInfoList(QDir::AllEntries | QDir::NoDotAndDotDot | QDir::Readable,
                                            QDir::DirsFirst | QDir::Name | QDir::IgnoreCase);

  beginResetModel();
  m_entries = QVector<QFileInfo>(entries.cbegin(), entries.cend());
  endResetModel();
  setError({});
}

void LocalFileModel::openRow(int row) {
  if (row < 0 || row >= m_entries.size()) {
    return;
  }
  const QFileInfo &entry = m_entries.at(row);
  if (entry.isDir()) {
    setPath(entry.absoluteFilePath());
  }
}

void LocalFileModel::goUp() {
  QDir dir(m_path);
  if (dir.cdUp()) {
    setPath(dir.absolutePath());
  }
}

bool LocalFileModel::createDirectory(const QString &name) {
  const QString trimmedName = name.trimmed();
  if (trimmedName.isEmpty()) {
    setError(tr("Folder name is required"));
    return false;
  }
  if (trimmedName.contains(QRegularExpression(QStringLiteral("[\\\\/]")))) {
    setError(tr("Folder name cannot contain path separators"));
    return false;
  }

  QDir dir(m_path);
  if (!dir.exists()) {
    setError(tr("Directory does not exist: %1").arg(m_path));
    return false;
  }
  if (!dir.mkdir(trimmedName)) {
    setError(tr("Could not create local folder: %1").arg(dir.filePath(trimmedName)));
    return false;
  }
  refresh();
  return true;
}

bool LocalFileModel::deletePath(const QString &path) {
  const QString trimmedPath = path.trimmed();
  if (trimmedPath.isEmpty()) {
    setError(tr("Select a local file or folder before deleting"));
    return false;
  }
  const QFileInfo target(trimmedPath);
  if (!target.exists()) {
    setError(tr("Local path does not exist: %1").arg(trimmedPath));
    return false;
  }
  const QString cleanTarget = QDir::cleanPath(target.absoluteFilePath());
  if (cleanTarget == QDir::cleanPath(m_path) || cleanTarget == QDir::rootPath() || cleanTarget == homePath()) {
    setError(tr("Refusing to delete this protected folder: %1").arg(cleanTarget));
    return false;
  }

  bool removed = false;
  if (target.isDir() && !target.isSymLink()) {
    QDir dir(cleanTarget);
    removed = dir.removeRecursively();
  } else {
    removed = QFile::remove(cleanTarget);
  }
  if (!removed) {
    setError(tr("Could not delete local path: %1").arg(cleanTarget));
    return false;
  }
  refresh();
  return true;
}

QString LocalFileModel::homePath() const {
  const QString home = QStandardPaths::writableLocation(QStandardPaths::HomeLocation);
  return home.isEmpty() ? QDir::homePath() : home;
}

void LocalFileModel::setError(const QString &error) {
  if (m_error == error) {
    return;
  }
  m_error = error;
  emit errorChanged();
}
