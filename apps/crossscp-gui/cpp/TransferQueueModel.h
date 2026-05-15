// SPDX-License-Identifier: AGPL-3.0-or-later

#pragma once

#include <QAbstractListModel>

#include "AppBackend.h"

class TransferQueueModel : public QAbstractListModel {
  Q_OBJECT

public:
  enum Roles { DirectionRole = Qt::UserRole + 1, SourceRole, DestinationRole, StateRole };

  explicit TransferQueueModel(QObject *parent = nullptr);

  int rowCount(const QModelIndex &parent = QModelIndex()) const override;
  QVariant data(const QModelIndex &index, int role = Qt::DisplayRole) const override;
  QHash<int, QByteArray> roleNames() const override;

  Q_INVOKABLE void setBackend(AppBackend *backend);
  Q_INVOKABLE bool enqueueLocalCopy(const QString &source, const QString &destination);
  Q_INVOKABLE void clearFinished();

private:
  struct Job {
    QString direction;
    QString source;
    QString destination;
    QString state;
  };

  QList<Job> jobs_;
  AppBackend *backend_ = nullptr;
};
