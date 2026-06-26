// SPDX-License-Identifier: AGPL-3.0-or-later

#include "TransferQueueModel.h"

#include <QCoreApplication>
#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QProcessEnvironment>
#include <QRegularExpression>
#include <QStandardPaths>
#include <QTextStream>

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

bool TransferQueueModel::useOpenSshBackend() const { return useOpenSshBackend_; }

void TransferQueueModel::setUseOpenSshBackend(bool enabled) {
  if (useOpenSshBackend_ == enabled) {
    return;
  }
  useOpenSshBackend_ = enabled;
  emit useOpenSshBackendChanged();
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
  return enqueueRemoteTransfer(QStringLiteral("Upload"), QStringLiteral("sftp"), host,
                               port, username, password, privateKeyPath,
                               privateKeyPassphrase, source, destination);
}

bool TransferQueueModel::enqueueSftpDownload(
    const QString &host, int port, const QString &username, const QString &password,
    const QString &privateKeyPath, const QString &privateKeyPassphrase,
    const QString &source, const QString &destination) {
  return enqueueRemoteTransfer(QStringLiteral("Download"), QStringLiteral("sftp"), host,
                               port, username, password, privateKeyPath,
                               privateKeyPassphrase, source, destination);
}

bool TransferQueueModel::enqueueRemoteUpload(
    const QString &protocol, const QString &host, int port, const QString &username,
    const QString &password, const QString &privateKeyPath,
    const QString &privateKeyPassphrase, const QString &source,
    const QString &destination) {
  return enqueueRemoteTransfer(QStringLiteral("Upload"), protocol, host, port, username,
                               password, privateKeyPath, privateKeyPassphrase, source,
                               destination);
}

bool TransferQueueModel::enqueueRemoteDownload(
    const QString &protocol, const QString &host, int port, const QString &username,
    const QString &password, const QString &privateKeyPath,
    const QString &privateKeyPassphrase, const QString &source,
    const QString &destination) {
  return enqueueRemoteTransfer(QStringLiteral("Download"), protocol, host, port,
                               username, password, privateKeyPath,
                               privateKeyPassphrase, source, destination);
}

bool TransferQueueModel::enqueueSftpTransfer(
    const QString &direction, const QString &host, int port, const QString &username,
    const QString &password, const QString &privateKeyPath,
    const QString &privateKeyPassphrase, const QString &source,
    const QString &destination) {
  return enqueueRemoteTransfer(direction, QStringLiteral("sftp"), host, port, username,
                               password, privateKeyPath, privateKeyPassphrase, source,
                               destination);
}

bool TransferQueueModel::enqueueRemoteTransfer(
    const QString &direction, const QString &protocol, const QString &host, int port,
    const QString &username, const QString &password, const QString &privateKeyPath,
    const QString &privateKeyPassphrase, const QString &source,
    const QString &destination) {
  if (backend_ == nullptr) {
    return false;
  }
  const QString normalizedProtocol = protocol.trimmed().toLower();
  if (host.trimmed().isEmpty() || username.trimmed().isEmpty() ||
      source.trimmed().isEmpty() || destination.trimmed().isEmpty() ||
      normalizedProtocol.isEmpty()) {
    return false;
  }

  const bool upload = direction == QStringLiteral("Upload");
  const bool useOpenSsh = useOpenSshBackend_ &&
                          (normalizedProtocol == QStringLiteral("sftp") ||
                           normalizedProtocol == QStringLiteral("scp"));
  QString program;
  QStringList arguments;
  if (useOpenSsh) {
    program = QStringLiteral("scp");
    arguments = openSshScpArguments(upload, host.trimmed(), port, username.trimmed(),
                                    privateKeyPath.trimmed(), source.trimmed(),
                                    destination.trimmed());
  } else {
    program = backend_->cliPath();
    arguments = {upload ? QStringLiteral("remote-upload")
                        : QStringLiteral("remote-download"),
                 QStringLiteral("--protocol"),
                 normalizedProtocol,
                 QStringLiteral("--host"),
                 host.trimmed(),
                 QStringLiteral("--port"),
                 QString::number(port),
                 QStringLiteral("--username"),
                 username.trimmed(),
                 upload ? QStringLiteral("--local") : QStringLiteral("--remote"),
                 source.trimmed(),
                 upload ? QStringLiteral("--remote") : QStringLiteral("--local"),
                 destination.trimmed()};
  }

  const int row = jobs_.size();
  beginInsertRows(QModelIndex(), row, row);
  Job job;
  job.direction = direction;
  job.protocol = normalizedProtocol;
  job.source = source.trimmed();
  job.destination = destination.trimmed();
  job.state = QStringLiteral("Queued");
  job.program = program;
  job.arguments = arguments;
  job.password = password;
  job.privateKeyPath = privateKeyPath.trimmed();
  job.privateKeyPassphrase = privateKeyPassphrase;
  job.usesOpenSsh = useOpenSsh;
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

QStringList TransferQueueModel::openSshScpArguments(
    bool upload, const QString &host, int port, const QString &username,
    const QString &privateKeyPath, const QString &source,
    const QString &destination) const {
  QStringList arguments = {QStringLiteral("-P"),
                           QString::number(port),
                           QStringLiteral("-p"),
                           QStringLiteral("-r"),
                           QStringLiteral("-o"),
                           QStringLiteral("StrictHostKeyChecking=accept-new"),
                           QStringLiteral("-o"),
                           QStringLiteral("NumberOfPasswordPrompts=1")};
  if (privateKeyPath.trimmed().isEmpty()) {
    arguments.append(QStringLiteral("-o"));
    arguments.append(QStringLiteral("BatchMode=no"));
  } else {
    arguments.append(QStringLiteral("-i"));
    arguments.append(privateKeyPath.trimmed());
    arguments.append(QStringLiteral("-o"));
    arguments.append(QStringLiteral("BatchMode=no"));
  }

  if (upload) {
    arguments.append(source);
    arguments.append(openSshRemoteSpec(username, host, destination));
  } else {
    arguments.append(openSshRemoteSpec(username, host, source));
    arguments.append(destination);
  }
  return arguments;
}

QString TransferQueueModel::openSshRemoteSpec(const QString &username,
                                              const QString &host,
                                              const QString &path) const {
  const QString cleanHost = host.contains(':') && !host.startsWith('[')
                                ? QStringLiteral("[%1]").arg(host)
                                : host;
  return QStringLiteral("%1@%2:%3").arg(username, cleanHost, path);
}

bool TransferQueueModel::prepareOpenSshAskPass(Job &job) {
  const QString secret = !job.password.isEmpty() ? job.password : job.privateKeyPassphrase;
  if (secret.isEmpty()) {
    return true;
  }

  const QString tempRoot = QStandardPaths::writableLocation(QStandardPaths::TempLocation);
  job.askPassPath = QDir(tempRoot).filePath(QStringLiteral("crossscp-openssh-askpass-%1-%2.sh")
                                                .arg(QCoreApplication::applicationPid())
                                                .arg(currentRow_));
  QFile askpass(job.askPassPath);
  if (!askpass.open(QIODevice::WriteOnly | QIODevice::Truncate | QIODevice::Text)) {
    job.askPassPath.clear();
    return false;
  }
  QTextStream stream(&askpass);
  stream << "#!/bin/sh\n";
  stream << "printf '%s\\n' \"$CROSSSCP_OPENSSH_SECRET\"\n";
  askpass.close();
  QFile::setPermissions(job.askPassPath, QFileDevice::ReadOwner |
                                             QFileDevice::WriteOwner |
                                             QFileDevice::ExeOwner);
  return true;
}

void TransferQueueModel::cleanupOpenSshAskPass(const Job &job) const {
  if (!job.askPassPath.isEmpty()) {
    QFile::remove(job.askPassPath);
  }
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
    if (currentRow_ >= 0 && currentRow_ < jobs_.size()) {
      cleanupOpenSshAskPass(jobs_.at(currentRow_));
    }
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
  if (job.usesOpenSsh) {
    if (!prepareOpenSshAskPass(job)) {
      failCurrentProcess(QStringLiteral("unable to create temporary OpenSSH askpass helper"));
      return;
    }
    const QString secret = !job.password.isEmpty() ? job.password : job.privateKeyPassphrase;
    if (!secret.isEmpty()) {
      environment.insert(QStringLiteral("SSH_ASKPASS"), job.askPassPath);
      environment.insert(QStringLiteral("SSH_ASKPASS_REQUIRE"), QStringLiteral("force"));
      environment.insert(QStringLiteral("DISPLAY"), environment.value(QStringLiteral("DISPLAY"), QStringLiteral("crossscp")));
      environment.insert(QStringLiteral("CROSSSCP_OPENSSH_SECRET"), secret);
    }
    job.state = QStringLiteral("Running with OpenSSH fast path");
    updateRow(currentRow_);
  } else if (!job.password.isEmpty()) {
    environment.insert(QStringLiteral("CROSSSCP_REMOTE_PASSWORD"), job.password);
    environment.insert(QStringLiteral("CROSSSCP_SFTP_PASSWORD"), job.password);
  }
  if (!job.usesOpenSsh && !job.privateKeyPath.isEmpty()) {
    environment.insert(QStringLiteral("CROSSSCP_REMOTE_PRIVATE_KEY_PATH"), job.privateKeyPath);
    environment.insert(QStringLiteral("CROSSSCP_SFTP_KEY_PATH"), job.privateKeyPath);
  }
  if (!job.usesOpenSsh && !job.privateKeyPassphrase.isEmpty()) {
    environment.insert(QStringLiteral("CROSSSCP_REMOTE_PRIVATE_KEY_PASSPHRASE"), job.privateKeyPassphrase);
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
              const QString program = currentRow_ >= 0 && currentRow_ < jobs_.size()
                                      ? jobs_.at(currentRow_).program
                                      : QStringLiteral("transfer process");
              failCurrentProcess(QStringLiteral("unable to start %1").arg(program));
            }
          });
  connect(currentProcess_, &QProcess::finished, this,
          &TransferQueueModel::finishCurrentProcess);

  currentProcess_->start(job.program, job.arguments);
}

void TransferQueueModel::consumeProgressOutput() {
  if (currentProcess_ == nullptr) {
    return;
  }
  progressBuffer_.append(currentProcess_->readAllStandardError());
  auto nextSeparator = [this]() -> int {
    const int newline = progressBuffer_.indexOf('\n');
    const int carriageReturn = progressBuffer_.indexOf('\r');
    if (newline < 0) {
      return carriageReturn;
    }
    if (carriageReturn < 0) {
      return newline;
    }
    return qMin(newline, carriageReturn);
  };

  int separator = nextSeparator();
  while (separator >= 0) {
    const QByteArray lineBytes = progressBuffer_.left(separator).trimmed();
    progressBuffer_.remove(0, separator + 1);
    const QString line = QString::fromUtf8(lineBytes);
    if (line.startsWith(QStringLiteral("progress\t"))) {
      processProgressLine(line);
    } else if (currentRow_ >= 0 && currentRow_ < jobs_.size() &&
               jobs_.at(currentRow_).usesOpenSsh &&
               processOpenSshProgressLine(line)) {
      // Parsed OpenSSH scp's human progress line; do not treat it as an error.
    } else if (!line.isEmpty()) {
      errorOutput_.append(lineBytes);
      errorOutput_.append('\n');
    }
    separator = nextSeparator();
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

bool TransferQueueModel::processOpenSshProgressLine(const QString &line) {
  if (currentRow_ < 0 || currentRow_ >= jobs_.size()) {
    return false;
  }
  const QString trimmed = line.trimmed();
  if (trimmed.isEmpty()) {
    return false;
  }

  static const QRegularExpression progressPattern(
      QStringLiteral(R"((\d{1,3})%\s+([0-9]+(?:\.[0-9]+)?)([KMGTPE]?i?B)\s+([0-9]+(?:\.[0-9]+)?)([KMGTPE]?i?B)/s)"),
      QRegularExpression::CaseInsensitiveOption);
  const QRegularExpressionMatch match = progressPattern.match(trimmed);
  if (!match.hasMatch()) {
    return false;
  }

  bool okPercent = false;
  const int percent = qBound(0, match.captured(1).toInt(&okPercent), 100);
  if (!okPercent) {
    return false;
  }

  const qlonglong transferredBytes = parseOpenSshByteAmount(match.captured(2), match.captured(3));
  const qlonglong speedBytesPerSecond = parseOpenSshSpeed(match.captured(4), match.captured(5));

  Job &job = jobs_[currentRow_];
  job.progress = percent;
  if (transferredBytes > 0) {
    job.bytesDone = transferredBytes;
  } else if (job.bytesTotal > 0) {
    job.bytesDone = static_cast<qlonglong>((job.bytesTotal * percent) / 100.0);
  }
  if (job.bytesTotal <= 0 && percent > 0 && job.bytesDone > 0) {
    job.bytesTotal = static_cast<qlonglong>((job.bytesDone * 100.0) / percent);
  }
  if (percent == 100 && job.bytesTotal < job.bytesDone) {
    job.bytesTotal = job.bytesDone;
  }
  if (speedBytesPerSecond > 0) {
    job.speedBytesPerSecond = speedBytesPerSecond;
  }

  if (job.bytesTotal > 0) {
    job.state = QStringLiteral("Running %1% (%2 / %3)")
                    .arg(job.progress)
                    .arg(formatBytes(job.bytesDone), formatBytes(job.bytesTotal));
  } else if (job.bytesDone > 0) {
    job.state = QStringLiteral("Running %1% (%2)")
                    .arg(job.progress)
                    .arg(formatBytes(job.bytesDone));
  } else {
    job.state = QStringLiteral("Running %1% with OpenSSH fast path").arg(job.progress);
  }
  updateRow(currentRow_);
  return true;
}

qlonglong TransferQueueModel::parseOpenSshByteAmount(const QString &value,
                                                     const QString &unit) const {
  bool ok = false;
  const double amount = value.toDouble(&ok);
  if (!ok || amount < 0) {
    return 0;
  }
  const QString normalized = unit.trimmed().toUpper();
  double multiplier = 1.0;
  if (normalized == QStringLiteral("KB") || normalized == QStringLiteral("KIB")) {
    multiplier = 1024.0;
  } else if (normalized == QStringLiteral("MB") || normalized == QStringLiteral("MIB")) {
    multiplier = 1024.0 * 1024.0;
  } else if (normalized == QStringLiteral("GB") || normalized == QStringLiteral("GIB")) {
    multiplier = 1024.0 * 1024.0 * 1024.0;
  } else if (normalized == QStringLiteral("TB") || normalized == QStringLiteral("TIB")) {
    multiplier = 1024.0 * 1024.0 * 1024.0 * 1024.0;
  } else if (normalized == QStringLiteral("PB") || normalized == QStringLiteral("PIB")) {
    multiplier = 1024.0 * 1024.0 * 1024.0 * 1024.0 * 1024.0;
  }
  return static_cast<qlonglong>(amount * multiplier);
}

qlonglong TransferQueueModel::parseOpenSshSpeed(const QString &value,
                                                const QString &unit) const {
  return parseOpenSshByteAmount(value, unit);
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
    } else if (currentRow_ >= 0 && currentRow_ < jobs_.size() &&
               jobs_.at(currentRow_).usesOpenSsh &&
               processOpenSshProgressLine(line)) {
      // Parsed trailing OpenSSH progress without a final newline/carriage return.
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
        error = QStringLiteral("%1 exited with code %2").arg(job.program, QString::number(exitCode));
      }
      markRowFailed(row, error);
      emit transferFailed(job.direction, job.source, job.destination, error);
    }
    cleanupOpenSshAskPass(job);
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
    cleanupOpenSshAskPass(jobs_.at(row));
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
