// SPDX-License-Identifier: AGPL-3.0-or-later

#include "RemoteFileModel.h"

#include <QDir>
#include <QRegularExpression>

RemoteFileModel::RemoteFileModel(QObject *parent) : QAbstractListModel(parent) {}

int RemoteFileModel::rowCount(const QModelIndex &parent) const {
  if (parent.isValid()) {
    return 0;
  }
  return entries_.size();
}

QVariant RemoteFileModel::data(const QModelIndex &index, int role) const {
  if (!index.isValid() || index.row() < 0 || index.row() >= entries_.size()) {
    return {};
  }
  const Entry &entry = entries_.at(index.row());
  switch (role) {
  case NameRole:
    return entry.name;
  case PathRole:
    return entry.path;
  case SizeRole:
    return entry.size;
  case IsDirectoryRole:
    return entry.isDirectory;
  default:
    return {};
  }
}

QHash<int, QByteArray> RemoteFileModel::roleNames() const {
  return {{NameRole, "name"}, {PathRole, "remotePath"}, {SizeRole, "size"},
          {IsDirectoryRole, "isDirectory"}};
}

QString RemoteFileModel::path() const { return path_; }

void RemoteFileModel::setPath(const QString &path) {
  if (path_ == path) {
    return;
  }
  path_ = path.isEmpty() ? QStringLiteral("/") : path;
  emit pathChanged();
}

QString RemoteFileModel::error() const { return error_; }

bool RemoteFileModel::connected() const { return connected_; }

void RemoteFileModel::setBackend(AppBackend *backend) { backend_ = backend; }

bool RemoteFileModel::connectPassword(const QString &host, int port, const QString &username,
                                      const QString &password, const QString &remotePath) {
  host_ = host.trimmed();
  port_ = port;
  username_ = username.trimmed();
  password_ = password;
  privateKeyPath_.clear();
  privateKeyPassphrase_.clear();
  if (password_.isEmpty()) {
    setError(QStringLiteral("Password is required for password authentication"));
    connected_ = false;
    emit connectedChanged();
    return false;
  }
  setPath(remotePath.trimmed().isEmpty() ? QStringLiteral("/") : remotePath.trimmed());
  connected_ = true;
  emit connectedChanged();
  refresh();
  return error_.isEmpty();
}

bool RemoteFileModel::connectKey(const QString &host, int port, const QString &username,
                                 const QString &privateKeyPath,
                                 const QString &privateKeyPassphrase,
                                 const QString &remotePath) {
  host_ = host.trimmed();
  port_ = port;
  username_ = username.trimmed();
  password_.clear();
  privateKeyPath_ = privateKeyPath.trimmed();
  privateKeyPassphrase_ = privateKeyPassphrase;
  if (privateKeyPath_.isEmpty()) {
    setError(QStringLiteral("Private key path is required for key authentication"));
    connected_ = false;
    emit connectedChanged();
    return false;
  }
  setPath(remotePath.trimmed().isEmpty() ? QStringLiteral("/") : remotePath.trimmed());
  connected_ = true;
  emit connectedChanged();
  refresh();
  return error_.isEmpty();
}

void RemoteFileModel::refresh() {
  if (backend_ == nullptr || host_.isEmpty() || username_.isEmpty()) {
    setError(QStringLiteral("Remote connection details are required"));
    return;
  }
  const CommandResult result = backend_->runCommand(
      {QStringLiteral("sftp-list"), host_, QString::number(port_), username_, path_}, password_,
      privateKeyPath_, privateKeyPassphrase_);
  if (result.exitCode != 0) {
    beginResetModel();
    entries_.clear();
    endResetModel();
    setError(result.standardError.trimmed());
    return;
  }
  QList<Entry> parsed;
  const QStringList lines = result.standardOutput.split('\n', Qt::SkipEmptyParts);
  for (const QString &line : lines) {
    const QStringList fields = line.split('\t');
    if (fields.size() < 4) {
      continue;
    }
    parsed.append({fields[3], fields[2], fields[1].toLongLong(), fields[0] == QStringLiteral("dir")});
  }
  beginResetModel();
  entries_ = parsed;
  endResetModel();
  setError(QString());
}

void RemoteFileModel::openRow(int row) {
  if (row < 0 || row >= entries_.size() || !entries_.at(row).isDirectory) {
    return;
  }
  setPath(entries_.at(row).path);
  refresh();
}

void RemoteFileModel::goUp() {
  QString cleanPath = QDir::cleanPath(path_.isEmpty() ? QStringLiteral("/") : path_);
  if (cleanPath == QStringLiteral(".") || cleanPath == QStringLiteral("/")) {
    setPath(QStringLiteral("/"));
    refresh();
    return;
  }

  const int separator = cleanPath.lastIndexOf('/');
  if (separator <= 0) {
    setPath(QStringLiteral("/"));
  } else {
    setPath(cleanPath.left(separator));
  }
  refresh();
}

void RemoteFileModel::disconnect() {
  beginResetModel();
  entries_.clear();
  endResetModel();
  host_.clear();
  username_.clear();
  password_.clear();
  privateKeyPath_.clear();
  privateKeyPassphrase_.clear();
  setPath(QStringLiteral("/"));
  setError(QString());
  if (connected_) {
    connected_ = false;
    emit connectedChanged();
  }
}

bool RemoteFileModel::uploadFile(const QString &localPath, const QString &remotePath) {
  if (backend_ == nullptr) {
    setError(QStringLiteral("backend unavailable"));
    return false;
  }
  if (localPath.trimmed().isEmpty()) {
    setError(QStringLiteral("Select a local file before uploading"));
    return false;
  }
  if (remotePath.trimmed().isEmpty()) {
    setError(QStringLiteral("Choose a remote destination path before uploading"));
    return false;
  }
  const CommandResult result = backend_->runCommand(
      {QStringLiteral("sftp-upload"), host_, QString::number(port_), username_, localPath.trimmed(),
       remotePath.trimmed()},
      password_, privateKeyPath_, privateKeyPassphrase_);
  if (result.exitCode != 0) {
    setError(result.standardError.trimmed());
    return false;
  }
  refresh();
  return true;
}

bool RemoteFileModel::downloadFile(const QString &remotePath, const QString &localPath) {
  if (backend_ == nullptr) {
    setError(QStringLiteral("backend unavailable"));
    return false;
  }
  const CommandResult result = backend_->runCommand(
      {QStringLiteral("sftp-download"), host_, QString::number(port_), username_, remotePath,
       localPath},
      password_, privateKeyPath_, privateKeyPassphrase_);
  if (result.exitCode != 0) {
    setError(result.standardError.trimmed());
    return false;
  }
  return true;
}

bool RemoteFileModel::createDirectory(const QString &name) {
  if (backend_ == nullptr) {
    setError(QStringLiteral("backend unavailable"));
    return false;
  }
  if (!connected_) {
    setError(QStringLiteral("Connect to SFTP before creating remote folders"));
    return false;
  }
  const QString trimmedName = name.trimmed();
  if (trimmedName.isEmpty()) {
    setError(QStringLiteral("Folder name is required"));
    return false;
  }
  if (trimmedName.contains(QRegularExpression(QStringLiteral("[\\\\/]")))) {
    setError(QStringLiteral("Folder name cannot contain path separators"));
    return false;
  }

  const QString remotePath = joinRemotePath(path_, trimmedName);
  const CommandResult result = backend_->runCommand(
      {QStringLiteral("sftp-mkdir"), host_, QString::number(port_), username_, remotePath},
      password_, privateKeyPath_, privateKeyPassphrase_);
  if (result.exitCode != 0) {
    setError(result.standardError.trimmed());
    return false;
  }
  refresh();
  return true;
}

bool RemoteFileModel::deletePath(const QString &remotePath) {
  if (backend_ == nullptr) {
    setError(QStringLiteral("backend unavailable"));
    return false;
  }
  if (!connected_) {
    setError(QStringLiteral("Connect to SFTP before deleting remote files"));
    return false;
  }
  const QString trimmedPath = remotePath.trimmed();
  if (trimmedPath.isEmpty()) {
    setError(QStringLiteral("Select a remote file or folder before deleting"));
    return false;
  }
  if (trimmedPath == QStringLiteral("/")) {
    setError(QStringLiteral("Refusing to delete the remote root folder"));
    return false;
  }

  const CommandResult result = backend_->runCommand(
      {QStringLiteral("sftp-delete"), host_, QString::number(port_), username_, trimmedPath},
      password_, privateKeyPath_, privateKeyPassphrase_);
  if (result.exitCode != 0) {
    setError(result.standardError.trimmed());
    return false;
  }
  refresh();
  return true;
}

void RemoteFileModel::setError(const QString &error) {
  if (error_ == error) {
    return;
  }
  error_ = error;
  emit errorChanged();
}

QString RemoteFileModel::joinRemotePath(const QString &basePath, const QString &name) const {
  const QString cleanBase = basePath.trimmed().isEmpty() ? QStringLiteral("/") : basePath.trimmed();
  if (cleanBase == QStringLiteral("/")) {
    return QStringLiteral("/") + name;
  }
  return cleanBase.endsWith(QLatin1Char('/')) ? cleanBase + name : cleanBase + QStringLiteral("/") + name;
}
