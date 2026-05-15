// SPDX-License-Identifier: AGPL-3.0-or-later

#include "TransferQueueModel.h"

TransferQueueModel::TransferQueueModel(QObject *parent) : QAbstractListModel(parent) {}

int TransferQueueModel::rowCount(const QModelIndex &parent) const {
  if (parent.isValid()) {
    return 0;
  }
  return jobs_.size();
}

QVariant TransferQueueModel::data(const QModelIndex &index, int role) const {
  if (!index.isValid() || index.row() < 0 || index.row() >= jobs_.size()) {
    return {};
  }
  const Job &job = jobs_.at(index.row());
  switch (role) {
  case DirectionRole:
    return job.direction;
  case SourceRole:
    return job.source;
  case DestinationRole:
    return job.destination;
  case StateRole:
    return job.state;
  default:
    return {};
  }
}

QHash<int, QByteArray> TransferQueueModel::roleNames() const {
  return {{DirectionRole, "direction"}, {SourceRole, "source"},
          {DestinationRole, "destination"}, {StateRole, "state"}};
}

void TransferQueueModel::setBackend(AppBackend *backend) { backend_ = backend; }

bool TransferQueueModel::enqueueLocalCopy(const QString &source, const QString &destination) {
  const int row = jobs_.size();
  beginInsertRows(QModelIndex(), row, row);
  jobs_.append({QStringLiteral("Local copy"), source, destination, QStringLiteral("Queued")});
  endInsertRows();

  if (backend_ == nullptr) {
    jobs_[row].state = QStringLiteral("Failed: backend unavailable");
    emit dataChanged(index(row), index(row));
    return false;
  }

  jobs_[row].state = QStringLiteral("Running");
  emit dataChanged(index(row), index(row));
  const bool ok = backend_->copyLocalFile(source, destination);
  jobs_[row].state = ok ? QStringLiteral("Completed") : QStringLiteral("Failed");
  emit dataChanged(index(row), index(row));
  return ok;
}

void TransferQueueModel::clearFinished() {
  beginResetModel();
  QList<Job> active;
  const QList<Job> current = jobs_;
  for (const Job &job : current) {
    if (job.state == QStringLiteral("Queued") || job.state == QStringLiteral("Running")) {
      active.append(job);
    }
  }
  jobs_ = active;
  endResetModel();
}
