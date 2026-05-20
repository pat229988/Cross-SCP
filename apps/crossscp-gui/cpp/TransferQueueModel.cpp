// SPDX-License-Identifier: AGPL-3.0-or-later

#include "TransferQueueModel.h"

#include <QDateTime>
#include <QFileInfo>
#include <QProcessEnvironment>

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
  case ProgressRole:
    return job.progress;
  case BytesDoneRole:
    return job.bytesDone;
  case BytesTotalRole:
    return job.bytesTotal;
  case ErrorRole:
    return job.error;
  case SpeedRole:
    return job.speedBytesPerSecond;
  case SpeedTextRole:
    return formatSpeed(job.speedBytesPerSecond);
  default:
    return {};
  }
}

QHash<int, QByteArray> TransferQueueModel::roleNames() const {
  return {{DirectionRole, "direction"}, {SourceRole, "source"},
          {DestinationRole, "destination"}, {StateRole, "state"},
          {ProgressRole, "progress"}, {BytesDoneRole, "bytesDone"},
          {BytesTotalRole, "bytesTotal"}, {ErrorRole, "error"},
          {SpeedRole, "speed"}, {SpeedTextRole, "speedText"}};
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
  jobs_[row].progress = 0;
  emit dataChanged(index(row), index(row));
  const bool ok = backend_->copyLocalFile(source, destination);
  jobs_[row].state = ok ? QStringLiteral("Completed") : QStringLiteral("Failed");
  jobs_[row].progress = ok ? 100 : 0;
  emit dataChanged(index(row), index(row));
  return ok;
}

bool TransferQueueModel::enqueueSftpUpload(
    const QString &host, int port, const QString &username, const QString &password,
    const QString &privateKeyPath, const QString &privateKeyPassphrase,
    const QString &source, const QString &destination) {
  return enqueueSftpTransfer(QStringLiteral("Upload"), host, port, username, password,
                             privateKeyPath, privateKeyPassphrase, source,
                             destination);
}

bool TransferQueueModel::enqueueSftpDownload(
    const QString &host, int port, const QString &username, const QString &password,
    const QString &privateKeyPath, const QString &privateKeyPassphrase,
    const QString &source, const QString &destination) {
  return enqueueSftpTransfer(QStringLiteral("Download"), host, port, username, password,
                             privateKeyPath, privateKeyPassphrase, source,
                             destination);
}

bool TransferQueueModel::enqueueSftpTransfer(
    const QString &direction, const QString &host, int port, const QString &username,
    const QString &password, const QString &privateKeyPath,
    const QString &privateKeyPassphrase, const QString &source,
    const QString &destination) {
  if (backend_ == nullptr) {
    return false;
  }
  if (host.trimmed().isEmpty() || username.trimmed().isEmpty() ||
      source.trimmed().isEmpty() || destination.trimmed().isEmpty()) {
    return false;
  }

  const bool upload = direction == QStringLiteral("Upload");
  QStringList arguments = {upload ? QStringLiteral("sftp-upload")
                                  : QStringLiteral("sftp-download"),
                           host.trimmed(),
                           QString::number(port),
                           username.trimmed(),
                           source.trimmed(),
                           destination.trimmed()};

  const int row = jobs_.size();
  beginInsertRows(QModelIndex(), row, row);
  Job job;
  job.direction = direction;
  job.source = source.trimmed();
  job.destination = destination.trimmed();
  job.state = QStringLiteral("Queued");
  job.arguments = arguments;
  job.password = password;
  job.privateKeyPath = privateKeyPath.trimmed();
  job.privateKeyPassphrase = privateKeyPassphrase;
  if (upload) {
    const QFileInfo sourceInfo(job.source);
    if (sourceInfo.isFile()) {
      job.bytesTotal = sourceInfo.size();
    }
  }
  jobs_.append(job);
  endInsertRows();

  startNextQueuedTransfer();
  return true;
}

void TransferQueueModel::clearFinished() {
  beginResetModel();
  QList<Job> active;
  const int oldCurrentRow = currentRow_;
  int newCurrentRow = -1;
  for (int row = 0; row < jobs_.size(); ++row) {
    const Job &job = jobs_.at(row);
    if (job.state == QStringLiteral("Queued") || job.state == QStringLiteral("Running")) {
      if (row == oldCurrentRow) {
        newCurrentRow = active.size();
      }
      active.append(job);
    }
  }
  jobs_ = active;
  currentRow_ = newCurrentRow;
  endResetModel();
}

void TransferQueueModel::clearAll() {
  if (currentProcess_ != nullptr) {
    currentProcess_->kill();
    currentProcess_->deleteLater();
    currentProcess_ = nullptr;
  }
  currentRow_ = -1;
  progressBuffer_.clear();
  errorOutput_.clear();
  standardOutput_.clear();
  beginResetModel();
  jobs_.clear();
  endResetModel();
}

void TransferQueueModel::startNextQueuedTransfer() {
  if (currentProcess_ != nullptr) {
    return;
  }
  for (int row = 0; row < jobs_.size(); ++row) {
    if (jobs_.at(row).state == QStringLiteral("Queued") &&
        !jobs_.at(row).arguments.isEmpty()) {
      currentRow_ = row;
      startCurrentProcess();
      return;
    }
  }
}

void TransferQueueModel::startCurrentProcess() {
  if (backend_ == nullptr || currentRow_ < 0 || currentRow_ >= jobs_.size()) {
    currentRow_ = -1;
    return;
  }

  Job &job = jobs_[currentRow_];
  job.state = QStringLiteral("Running");
  job.error.clear();
  job.startedAtMs = QDateTime::currentMSecsSinceEpoch();
  job.speedBytesPerSecond = 0;
  updateRow(currentRow_);

  progressBuffer_.clear();
  errorOutput_.clear();
  standardOutput_.clear();

  currentProcess_ = new QProcess(this);
  QProcessEnvironment environment = QProcessEnvironment::systemEnvironment();
  if (!job.password.isEmpty()) {
    environment.insert(QStringLiteral("CROSSSCP_SFTP_PASSWORD"), job.password);
  }
  if (!job.privateKeyPath.isEmpty()) {
    environment.insert(QStringLiteral("CROSSSCP_SFTP_KEY_PATH"), job.privateKeyPath);
  }
  if (!job.privateKeyPassphrase.isEmpty()) {
    environment.insert(QStringLiteral("CROSSSCP_SFTP_KEY_PASSPHRASE"), job.privateKeyPassphrase);
  }
  currentProcess_->setProcessEnvironment(environment);

  connect(currentProcess_, &QProcess::started, this,
          [this]() { currentProcess_->closeWriteChannel(); });
  connect(currentProcess_, &QProcess::readyReadStandardError, this,
          &TransferQueueModel::consumeProgressOutput);
  connect(currentProcess_, &QProcess::readyReadStandardOutput, this, [this]() {
    if (currentProcess_ != nullptr) {
      standardOutput_.append(currentProcess_->readAllStandardOutput());
    }
  });
  connect(currentProcess_, &QProcess::errorOccurred, this,
          [this](QProcess::ProcessError error) {
            if (error == QProcess::FailedToStart) {
              failCurrentProcess(QStringLiteral("unable to start crossscp CLI"));
            }
          });
  connect(currentProcess_, &QProcess::finished, this,
          &TransferQueueModel::finishCurrentProcess);

  currentProcess_->start(backend_->cliPath(), job.arguments);
}

void TransferQueueModel::consumeProgressOutput() {
  if (currentProcess_ == nullptr) {
    return;
  }
  progressBuffer_.append(currentProcess_->readAllStandardError());
  int newline = progressBuffer_.indexOf('\n');
  while (newline >= 0) {
    const QByteArray lineBytes = progressBuffer_.left(newline).trimmed();
    progressBuffer_.remove(0, newline + 1);
    const QString line = QString::fromUtf8(lineBytes);
    if (line.startsWith(QStringLiteral("progress\t"))) {
      processProgressLine(line);
    } else if (!line.isEmpty()) {
      errorOutput_.append(lineBytes);
      errorOutput_.append('\n');
    }
    newline = progressBuffer_.indexOf('\n');
  }
}

void TransferQueueModel::processProgressLine(const QString &line) {
  if (currentRow_ < 0 || currentRow_ >= jobs_.size()) {
    return;
  }
  const QStringList fields = line.split('\t');
  if (fields.size() < 3) {
    return;
  }
  bool okDone = false;
  bool okTotal = false;
  const qlonglong bytesDone = fields[1].toLongLong(&okDone);
  const qlonglong bytesTotal = fields[2].toLongLong(&okTotal);
  if (!okDone) {
    return;
  }

  Job &job = jobs_[currentRow_];
  job.bytesDone = bytesDone;
  const qint64 elapsedMs = QDateTime::currentMSecsSinceEpoch() - job.startedAtMs;
  if (elapsedMs > 0 && bytesDone > 0) {
    job.speedBytesPerSecond = static_cast<qlonglong>((bytesDone * 1000.0) / elapsedMs);
  }
  if (okTotal && bytesTotal > 0) {
    job.bytesTotal = bytesTotal;
    job.progress = qBound(0, static_cast<int>((bytesDone * 100) / bytesTotal), 100);
    job.state = QStringLiteral("Running %1% (%2 / %3)")
                    .arg(job.progress)
                    .arg(formatBytes(bytesDone), formatBytes(bytesTotal));
  } else {
    job.state = QStringLiteral("Running (%1)").arg(formatBytes(bytesDone));
  }
  updateRow(currentRow_);
}

void TransferQueueModel::finishCurrentProcess(int exitCode,
                                              QProcess::ExitStatus exitStatus) {
  if (currentProcess_ == nullptr) {
    return;
  }
  consumeProgressOutput();
  if (!progressBuffer_.trimmed().isEmpty()) {
    const QString line = QString::fromUtf8(progressBuffer_.trimmed());
    if (line.startsWith(QStringLiteral("progress\t"))) {
      processProgressLine(line);
    } else {
      errorOutput_.append(progressBuffer_.trimmed());
      errorOutput_.append('\n');
    }
  }

  const int row = currentRow_;
  QProcess *process = currentProcess_;
  currentProcess_ = nullptr;
  currentRow_ = -1;

  if (row >= 0 && row < jobs_.size()) {
    Job &job = jobs_[row];
    if (exitStatus == QProcess::NormalExit && exitCode == 0) {
      job.progress = 100;
      if (job.bytesTotal > 0 && job.bytesDone < job.bytesTotal) {
        job.bytesDone = job.bytesTotal;
      }
      job.state = job.bytesTotal > 0
                      ? QStringLiteral("Completed (%1)").arg(formatBytes(job.bytesTotal))
                      : QStringLiteral("Completed");
      updateRow(row);
      emit transferCompleted(job.direction, job.source, job.destination);
    } else {
      QString error = QString::fromUtf8(errorOutput_).trimmed();
      if (error.isEmpty()) {
        error = QStringLiteral("crossscp CLI exited with code %1").arg(exitCode);
      }
      markRowFailed(row, error);
      emit transferFailed(job.direction, job.source, job.destination, error);
    }
  }

  process->deleteLater();
  progressBuffer_.clear();
  errorOutput_.clear();
  standardOutput_.clear();
  startNextQueuedTransfer();
}

void TransferQueueModel::failCurrentProcess(const QString &error) {
  if (currentProcess_ == nullptr) {
    return;
  }
  const int row = currentRow_;
  QProcess *process = currentProcess_;
  currentProcess_ = nullptr;
  currentRow_ = -1;
  if (row >= 0 && row < jobs_.size()) {
    markRowFailed(row, error);
    emit transferFailed(jobs_.at(row).direction, jobs_.at(row).source,
                        jobs_.at(row).destination, error);
  }
  process->deleteLater();
  startNextQueuedTransfer();
}

void TransferQueueModel::updateRow(int row) {
  if (row < 0 || row >= jobs_.size()) {
    return;
  }
  emit dataChanged(index(row), index(row));
}

void TransferQueueModel::markRowFailed(int row, const QString &error) {
  if (row < 0 || row >= jobs_.size()) {
    return;
  }
  jobs_[row].state = QStringLiteral("Failed");
  jobs_[row].error = error;
  updateRow(row);
}

QString TransferQueueModel::formatBytes(qlonglong bytes) const {
  static const QStringList units = {QStringLiteral("B"), QStringLiteral("KiB"),
                                   QStringLiteral("MiB"), QStringLiteral("GiB"),
                                   QStringLiteral("TiB")};
  double value = static_cast<double>(bytes);
  int unit = 0;
  while (value >= 1024.0 && unit < units.size() - 1) {
    value /= 1024.0;
    ++unit;
  }
  return unit == 0 ? QStringLiteral("%1 %2").arg(bytes).arg(units[unit])
                   : QStringLiteral("%1 %2").arg(value, 0, 'f', 1).arg(units[unit]);
}

QString TransferQueueModel::formatSpeed(qlonglong bytesPerSecond) const {
  if (bytesPerSecond <= 0) {
    return QStringLiteral("—");
  }
  return QStringLiteral("%1/s").arg(formatBytes(bytesPerSecond));
}
