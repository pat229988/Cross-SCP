// SPDX-License-Identifier: AGPL-3.0-or-later

#include <QGuiApplication>
#include <QIcon>
#include <QQmlApplicationEngine>

#include "AppBackend.h"
#include "LocalFileModel.h"
#include "RemoteFileModel.h"
#include "TransferQueueModel.h"

int main(int argc, char *argv[]) {
  QGuiApplication app(argc, argv);
  QGuiApplication::setApplicationName("CrossSCP");
  QGuiApplication::setOrganizationName("CrossSCP");
  QGuiApplication::setWindowIcon(
      QIcon(QStringLiteral(":/qt/qml/CrossSCP/resources/icons/crossscp-256.png")));

  qmlRegisterType<AppBackend>("CrossSCP.Models", 1, 0, "AppBackend");
  qmlRegisterType<LocalFileModel>("CrossSCP.Models", 1, 0, "LocalFileModel");
  qmlRegisterType<RemoteFileModel>("CrossSCP.Models", 1, 0, "RemoteFileModel");
  qmlRegisterType<TransferQueueModel>("CrossSCP.Models", 1, 0,
                                      "TransferQueueModel");

  QQmlApplicationEngine engine;
  engine.loadFromModule("CrossSCP", "Main");
  if (engine.rootObjects().isEmpty()) {
    return 1;
  }

  return QGuiApplication::exec();
}
