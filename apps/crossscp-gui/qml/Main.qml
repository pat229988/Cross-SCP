// SPDX-License-Identifier: AGPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import CrossSCP.Models 1.0

ApplicationWindow {
    id: root
    width: 1280
    height: 800
    visible: true
    title: qsTr("CrossSCP")

    property string statusText: backend.status
    property var savedSites: []
    property string selectedSiteLine: ""
    property string selectedLocalPath: ""
    property string selectedLocalName: ""
    property bool selectedLocalIsDirectory: false
    property string selectedRemotePath: ""
    property string selectedRemoteName: ""
    property bool selectedRemoteIsDirectory: false
    property string transferLocalPath: ""
    property string transferRemotePath: ""
    property bool newFolderRemote: false
    property bool deleteRemote: false
    property string deleteTargetPath: ""
    property var scannedSshKeys: []
    property var selectedLocalItems: []
    property var selectedRemoteItems: []

    function addLog(message) {
        var now = new Date()
        sessionLogModel.append({ timestamp: now.toLocaleTimeString(), message: message })
        if (sessionLogModel.count > 250) {
            sessionLogModel.remove(0)
        }
    }

    function localItemSelected(path) {
        return selectedLocalItems.some(function(item) { return item.path === path })
    }

    function remoteItemSelected(path) {
        return selectedRemoteItems.some(function(item) { return item.path === path })
    }

    function toggleLocalSelection(item) {
        var items = selectedLocalItems.slice()
        var existing = items.findIndex(function(entry) { return entry.path === item.path })
        if (existing >= 0) {
            items.splice(existing, 1)
        } else {
            items.push(item)
        }
        selectedLocalItems = items
        if (items.length > 0) {
            var latest = item
            selectedLocalPath = latest.path
            selectedLocalName = latest.name
            selectedLocalIsDirectory = latest.isDirectory
            transferLocalPath = items.length === 1 ? latest.path : qsTr("%1 local items selected").arg(items.length)
            transferRemotePath = items.length === 1 ? joinRemotePath(remoteModel.path, latest.name) : remoteModel.path
        }
    }

    function toggleRemoteSelection(item) {
        var items = selectedRemoteItems.slice()
        var existing = items.findIndex(function(entry) { return entry.path === item.path })
        if (existing >= 0) {
            items.splice(existing, 1)
        } else {
            items.push(item)
        }
        selectedRemoteItems = items
        if (items.length > 0) {
            var latest = item
            selectedRemotePath = latest.path
            selectedRemoteName = latest.name
            selectedRemoteIsDirectory = latest.isDirectory
            transferRemotePath = items.length === 1 ? latest.path : qsTr("%1 remote items selected").arg(items.length)
            transferLocalPath = items.length === 1 ? joinLocalPath(leftModel.path, latest.name) : leftModel.path
        }
    }

    function clearLocalSelection() {
        selectedLocalItems = []
        selectedLocalPath = ""
        selectedLocalName = ""
        transferLocalPath = ""
    }

    function clearRemoteSelection() {
        selectedRemoteItems = []
        selectedRemotePath = ""
        selectedRemoteName = ""
        transferRemotePath = ""
    }

    function joinRemotePath(basePath, fileName) {
        if (!fileName || fileName.length === 0) {
            return basePath && basePath.length > 0 ? basePath : "/"
        }
        var base = basePath && basePath.length > 0 ? basePath : "/"
        if (base === "/") {
            return "/" + fileName
        }
        return base.endsWith("/") ? base + fileName : base + "/" + fileName
    }

    function normalizeUploadRemotePath(inputPath, currentRemotePath, localName) {
        var target = inputPath && inputPath.trim().length > 0 ? inputPath.trim() : ""
        if (target.length === 0) {
            return joinRemotePath(currentRemotePath, localName)
        }
        if (target === ".") {
            return joinRemotePath(currentRemotePath, localName)
        }
        if (target.endsWith("/")) {
            return joinRemotePath(target, localName)
        }
        if (!target.startsWith("/")) {
            return joinRemotePath(currentRemotePath, target)
        }
        return target
    }

    function joinLocalPath(basePath, fileName) {
        if (!fileName || fileName.length === 0) {
            return basePath
        }
        if (!basePath || basePath.length === 0 || basePath === "/") {
            return "/" + fileName
        }
        return basePath.endsWith("/") ? basePath + fileName : basePath + "/" + fileName
    }

    function fileNameFromPath(path) {
        if (!path || path.length === 0) {
            return ""
        }
        var normalized = path.replace(/\\/g, "/")
        var slash = normalized.lastIndexOf("/")
        return slash >= 0 ? normalized.substring(slash + 1) : normalized
    }

    function prepareUploadRemotePath(localPath) {
        var localName = root.selectedLocalName.length > 0 ? root.selectedLocalName : root.fileNameFromPath(localPath)
        if (root.transferRemotePath.length > 0 && root.transferRemotePath !== root.selectedRemotePath) {
            return root.normalizeUploadRemotePath(root.transferRemotePath, remoteModel.path, localName)
        }
        if (root.selectedRemoteIsDirectory && root.selectedRemotePath.length > 0) {
            return root.joinRemotePath(root.selectedRemotePath, localName)
        }
        return root.joinRemotePath(remoteModel.path, localName)
    }

    function performUpload() {
        if (selectedLocalItems.length > 0) {
            var uploaded = 0
            var baseRemotePath = selectedRemoteIsDirectory && selectedRemotePath.length > 0 ? selectedRemotePath : remoteModel.path
            for (var i = 0; i < selectedLocalItems.length; i++) {
                var item = selectedLocalItems[i]
                var destination = joinRemotePath(baseRemotePath, item.name)
                if (remoteModel.uploadFile(item.path, destination)) {
                    uploaded++
                    addLog(qsTr("Uploaded %1 → %2").arg(item.path).arg(destination))
                } else {
                    addLog(qsTr("Upload failed: %1").arg(item.path))
                }
            }
            remoteModel.refresh()
            statusText = qsTr("Uploaded %1 of %2 selected items").arg(uploaded).arg(selectedLocalItems.length)
            return
        }
        var localPath = root.transferLocalPath.length > 0 ? root.transferLocalPath : root.selectedLocalPath
        if (localPath.length === 0) {
            statusText = qsTr("Select a local file or folder before uploading")
            return
        }
        var remotePath = root.prepareUploadRemotePath(localPath)
        if (remoteModel.uploadFile(localPath, remotePath)) {
            root.transferRemotePath = remotePath
            statusText = qsTr("Uploaded %1").arg(localPath)
            addLog(qsTr("Uploaded %1 → %2").arg(localPath).arg(remotePath))
        }
    }

    function performDownload() {
        if (selectedRemoteItems.length > 0) {
            var downloaded = 0
            for (var i = 0; i < selectedRemoteItems.length; i++) {
                var item = selectedRemoteItems[i]
                var destination = joinLocalPath(leftModel.path, item.name)
                if (remoteModel.downloadFile(item.path, destination)) {
                    downloaded++
                    addLog(qsTr("Downloaded %1 → %2").arg(item.path).arg(destination))
                } else {
                    addLog(qsTr("Download failed: %1").arg(item.path))
                }
            }
            leftModel.refresh()
            statusText = qsTr("Downloaded %1 of %2 selected items").arg(downloaded).arg(selectedRemoteItems.length)
            return
        }
        var remotePath = root.transferRemotePath.length > 0 ? root.transferRemotePath : root.selectedRemotePath
        if (remotePath.length === 0) {
            statusText = qsTr("Select a remote file or folder before downloading")
            return
        }
        var localName = root.selectedRemoteName.length > 0 ? root.selectedRemoteName : root.fileNameFromPath(remotePath)
        var localPath = root.transferLocalPath.length > 0 ? root.transferLocalPath : root.joinLocalPath(leftModel.path, localName)
        if (remoteModel.downloadFile(remotePath, localPath)) {
            root.transferLocalPath = localPath
            leftModel.refresh()
            statusText = qsTr("Downloaded %1").arg(remotePath)
            addLog(qsTr("Downloaded %1 → %2").arg(remotePath).arg(localPath))
        }
    }

    function runPaneAction(actionName) {
        if (actionName === qsTr("Download") || actionName === "Download") {
            performDownload()
        } else {
            performUpload()
        }
    }

    function openNewFolderDialog(remote) {
        root.newFolderRemote = remote
        newFolderNameField.text = ""
        newFolderDialog.open()
        newFolderNameField.forceActiveFocus()
    }

    function openDeleteDialog(remote) {
        root.deleteRemote = remote
        if (remote && selectedRemoteItems.length > 0) {
            root.deleteTargetPath = selectedRemoteItems.length === 1 ? selectedRemoteItems[0].path : qsTr("%1 selected remote items").arg(selectedRemoteItems.length)
        } else if (!remote && selectedLocalItems.length > 0) {
            root.deleteTargetPath = selectedLocalItems.length === 1 ? selectedLocalItems[0].path : qsTr("%1 selected local items").arg(selectedLocalItems.length)
        } else {
            root.deleteTargetPath = remote ? (root.selectedRemotePath.length > 0 ? root.selectedRemotePath : root.transferRemotePath) : (root.selectedLocalPath.length > 0 ? root.selectedLocalPath : root.transferLocalPath)
        }
        if (root.deleteTargetPath.length === 0) {
            statusText = remote ? qsTr("Select a remote file or folder before deleting") : qsTr("Select a local file or folder before deleting")
            return
        }
        deleteConfirmDialog.open()
    }

    ListModel { id: sessionLogModel }

    AppBackend { id: backend }
    LocalFileModel { id: leftModel }
    RemoteFileModel { id: remoteModel; Component.onCompleted: setBackend(backend) }
    TransferQueueModel { id: queueModel; Component.onCompleted: setBackend(backend) }

    FileDialog {
        id: privateKeyFileDialog
        title: qsTr("Select SSH private key")
        onAccepted: sitePrivateKeyField.text = backend.localPathFromUrl(String(selectedFile))
    }

    Dialog {
        id: newFolderDialog
        title: root.newFolderRemote ? qsTr("New Remote Folder") : qsTr("New Local Folder")
        modal: true
        standardButtons: Dialog.Ok | Dialog.Cancel
        anchors.centerIn: parent
        width: Math.min(root.width * 0.44, 460)
        contentItem: ColumnLayout {
            spacing: 10
            Label {
                Layout.fillWidth: true
                text: root.newFolderRemote ? qsTr("Create a folder in %1").arg(remoteModel.path) : qsTr("Create a folder in %1").arg(leftModel.path)
                wrapMode: Text.WordWrap
            }
            TextField {
                id: newFolderNameField
                Layout.fillWidth: true
                placeholderText: qsTr("Folder name")
                selectByMouse: true
                onAccepted: newFolderDialog.accept()
            }
        }
        onAccepted: {
            if (root.newFolderRemote) {
                if (remoteModel.createDirectory(newFolderNameField.text)) {
                    root.addLog(qsTr("Created remote folder %1").arg(root.joinRemotePath(remoteModel.path, newFolderNameField.text)))
                }
            } else {
                if (leftModel.createDirectory(newFolderNameField.text)) {
                    root.addLog(qsTr("Created local folder %1").arg(root.joinLocalPath(leftModel.path, newFolderNameField.text)))
                }
            }
        }
    }

    Dialog {
        id: deleteConfirmDialog
        title: root.deleteRemote ? qsTr("Delete Remote Item") : qsTr("Delete Local Item")
        modal: true
        standardButtons: Dialog.Yes | Dialog.No
        anchors.centerIn: parent
        width: Math.min(root.width * 0.46, 500)
        contentItem: ColumnLayout {
            spacing: 10
            Label {
                Layout.fillWidth: true
                text: qsTr("Delete this file or folder? This cannot be undone.")
                wrapMode: Text.WordWrap
                font.bold: true
            }
            Label {
                Layout.fillWidth: true
                text: root.deleteTargetPath
                wrapMode: Text.WrapAnywhere
                color: "#b00020"
            }
        }
        onAccepted: {
            if (root.deleteRemote) {
                if (root.selectedRemoteItems.length > 0) {
                    var remoteDeleted = 0
                    for (var i = 0; i < root.selectedRemoteItems.length; i++) {
                        var remoteItem = root.selectedRemoteItems[i]
                        if (remoteModel.deletePath(remoteItem.path)) {
                            remoteDeleted++
                            root.addLog(qsTr("Deleted remote %1").arg(remoteItem.path))
                        }
                    }
                    root.clearRemoteSelection()
                    statusText = qsTr("Deleted %1 selected remote items").arg(remoteDeleted)
                } else if (remoteModel.deletePath(root.deleteTargetPath)) {
                    root.addLog(qsTr("Deleted remote %1").arg(root.deleteTargetPath))
                    root.selectedRemotePath = ""
                    root.selectedRemoteName = ""
                    root.transferRemotePath = ""
                    statusText = qsTr("Deleted remote item")
                }
            } else {
                if (root.selectedLocalItems.length > 0) {
                    var localDeleted = 0
                    for (var j = 0; j < root.selectedLocalItems.length; j++) {
                        var localItem = root.selectedLocalItems[j]
                        if (leftModel.deletePath(localItem.path)) {
                            localDeleted++
                            root.addLog(qsTr("Deleted local %1").arg(localItem.path))
                        }
                    }
                    root.clearLocalSelection()
                    statusText = qsTr("Deleted %1 selected local items").arg(localDeleted)
                } else if (leftModel.deletePath(root.deleteTargetPath)) {
                    root.addLog(qsTr("Deleted local %1").arg(root.deleteTargetPath))
                    root.selectedLocalPath = ""
                    root.selectedLocalName = ""
                    root.transferLocalPath = ""
                    statusText = qsTr("Deleted local item")
                }
            }
        }
    }

    header: ToolBar {
        RowLayout {
            anchors.fill: parent
            spacing: 10

            Image {
                source: "qrc:/qt/qml/CrossSCP/resources/icons/crossscp-256.png"
                sourceSize.width: 30
                sourceSize.height: 30
                Layout.preferredWidth: 30
                Layout.preferredHeight: 30
                Layout.maximumWidth: 30
                Layout.maximumHeight: 30
                width: 30
                height: 30
                fillMode: Image.PreserveAspectFit
                Accessible.name: qsTr("CrossSCP logo")
                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: Qt.openUrlExternally("https://github.com/pat229988/Cross-SCP.git")
                }
            }

            Label {
                text: qsTr("CrossSCP")
                font.pixelSize: 18
                font.bold: true
            }

            ToolButton {
                text: qsTr("Sites")
                Layout.minimumWidth: 84
                onClicked: siteManagerDialog.open()
                Accessible.name: qsTr("Open Site Manager")
            }

            ToolButton {
                text: qsTr("Connect")
                Layout.minimumWidth: 96
                onClicked: siteManagerDialog.open()
                Accessible.name: qsTr("Connect to selected site")
            }

            ToolButton {
                text: qsTr("Disconnect")
                Layout.minimumWidth: 108
                enabled: remoteModel.connected
                onClicked: {
                    remoteModel.disconnect()
                    root.clearRemoteSelection()
                    root.transferRemotePath = ""
                    root.addLog(qsTr("Disconnected from remote SFTP session"))
                    statusText = qsTr("Disconnected")
                }
                Accessible.name: qsTr("Disconnect from remote server")
            }

            ToolButton {
                text: qsTr("About")
                Layout.minimumWidth: 84
                onClicked: aboutDialog.open()
                Accessible.name: qsTr("Open About dialog")
            }

            Label {
                Layout.fillWidth: true
                text: statusText
                horizontalAlignment: Text.AlignRight
                elide: Text.ElideRight
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 6

        SplitView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumHeight: 430
            orientation: Qt.Horizontal

            FilePane {
                SplitView.preferredWidth: root.width / 2
                SplitView.minimumWidth: 420
                title: qsTr("Local")
                subtitle: qsTr("Local filesystem")
                fileModel: leftModel
                paneAccent: "#0b6bcb"
            }

            RemotePane {
                SplitView.preferredWidth: root.width / 2
                SplitView.minimumWidth: 520
                title: remoteModel.connected ? qsTr("Remote SFTP") : qsTr("Remote SFTP")
                subtitle: remoteModel.connected ? qsTr("Connected through Rust CLI SFTP bridge") : qsTr("Open Sites, choose password or SSH key auth, then Connect")
                remoteFileModel: remoteModel
                paneAccent: "#6b46c1"
            }
        }

        SplitView {
            Layout.fillWidth: true
            Layout.preferredHeight: 240
            Layout.minimumHeight: 140
            orientation: Qt.Vertical

            QueueStrip {
                SplitView.fillWidth: true
                SplitView.preferredHeight: 95
                SplitView.minimumHeight: 56
            }

            LogsPanel {
                SplitView.fillWidth: true
                SplitView.preferredHeight: 145
                SplitView.minimumHeight: 70
            }
        }
    }

    footer: Frame {
        RowLayout {
            anchors.fill: parent
            Label {
                Layout.fillWidth: true
                text: statusText
                elide: Text.ElideRight
            }
        }
    }

    Dialog {
        id: siteManagerDialog
        title: qsTr("Site Manager")
        modal: true
        standardButtons: Dialog.Close
        width: Math.min(root.width * 0.9, 980)
        height: Math.min(root.height * 0.86, 700)
        anchors.centerIn: parent

        ColumnLayout {
            anchors.fill: parent
            spacing: 12

            Label {
                Layout.fillWidth: true
                text: qsTr("Create, update, delete, and connect SFTP session profiles. Profiles are persisted by the Rust config service. Passwords are used only for this connection and are not saved.")
                wrapMode: Text.WordWrap
            }

            ListView {
                id: sitesList
                Layout.fillWidth: true
                Layout.preferredHeight: 110
                clip: true
                model: savedSites
                delegate: ItemDelegate {
                    width: ListView.view.width
                    text: modelData.length > 0 ? modelData.split("\t")[0] + "  —  " + modelData.split("\t")[2] : ""
                    onClicked: {
                        selectedSiteLine = modelData
                        var fields = modelData.split("\t")
                        siteNameField.text = fields[0] || ""
                        siteHostField.text = fields[2] || ""
                        sitePortField.value = Number(fields[3] || 22)
                        siteUsernameField.text = fields[4] || ""
                        siteRemotePathField.text = fields[5] || "/"
                        siteCredentialRefField.text = fields[6] || ""
                    }
                }
            }

            GridLayout {
                Layout.fillWidth: true
                columns: 2

                Label { text: qsTr("Protocol") }
                ComboBox {
                    id: protocolCombo
                    Layout.fillWidth: true
                    model: ["SFTP"]
                    ToolTip.visible: hovered
                    ToolTip.text: qsTr("V1 live connectivity uses the SFTP backend. SCP/FTP/FTPS/WebDAV/S3 are modeled in core and will get live adapters after SFTP stabilizes.")
                }

                Label { text: qsTr("Authentication") }
                ComboBox { id: authMethodCombo; Layout.fillWidth: true; model: ["Password", "SSH Private Key"] }

                Label { text: qsTr("Profile Name") }
                TextField { id: siteNameField; Layout.fillWidth: true; placeholderText: qsTr("Production") }

                Label { text: qsTr("Host") }
                TextField { id: siteHostField; Layout.fillWidth: true; placeholderText: qsTr("sftp.example.com") }

                Label { text: qsTr("Port") }
                SpinBox { id: sitePortField; from: 1; to: 65535; value: 22 }

                Label { text: qsTr("Username") }
                TextField { id: siteUsernameField; Layout.fillWidth: true; placeholderText: qsTr("alice") }

                Label { text: qsTr("Remote Path") }
                TextField { id: siteRemotePathField; Layout.fillWidth: true; text: "/" }

                Label { text: qsTr("Credential Reference") }
                TextField { id: siteCredentialRefField; Layout.fillWidth: true; placeholderText: qsTr("keychain://site-name") }

                Label { text: qsTr("Password (not saved)") }
                TextField {
                    id: sitePasswordField
                    Layout.fillWidth: true
                    enabled: authMethodCombo.currentText === "Password"
                    echoMode: TextInput.Password
                    placeholderText: qsTr("SFTP password for this connection")
                }

                Label { text: qsTr("Private Key") }
                RowLayout {
                    Layout.fillWidth: true
                    ComboBox {
                        id: sshKeyCombo
                        Layout.preferredWidth: 250
                        enabled: authMethodCombo.currentText === "SSH Private Key"
                        model: root.scannedSshKeys
                        displayText: currentText.length > 0 ? root.fileNameFromPath(currentText) : qsTr("Scanned ~/.ssh keys")
                        onActivated: sitePrivateKeyField.text = currentText
                    }
                    TextField {
                        id: sitePrivateKeyField
                        Layout.fillWidth: true
                        enabled: authMethodCombo.currentText === "SSH Private Key"
                        placeholderText: qsTr("Choose scanned key, browse, or paste key path")
                    }
                    Button {
                        text: qsTr("Browse…")
                        Layout.minimumWidth: 96
                        enabled: authMethodCombo.currentText === "SSH Private Key"
                        onClicked: privateKeyFileDialog.open()
                    }
                }

                Label { text: qsTr("Key Passphrase") }
                TextField {
                    id: sitePrivateKeyPassphraseField
                    Layout.fillWidth: true
                    enabled: authMethodCombo.currentText === "SSH Private Key"
                    echoMode: TextInput.Password
                    placeholderText: qsTr("Optional private key passphrase")
                }
            }

            GridLayout {
                Layout.fillWidth: true
                columns: 5
                Button {
                    text: qsTr("Reload")
                    Layout.fillWidth: true
                    onClicked: savedSites = backend.listSites()
                }
                Button {
                    text: qsTr("Save")
                    Layout.fillWidth: true
                    onClicked: {
                        if (backend.saveSite(siteNameField.text, siteHostField.text, sitePortField.value, siteUsernameField.text, siteRemotePathField.text, siteCredentialRefField.text)) {
                            savedSites = backend.listSites()
                            root.addLog(qsTr("Saved site profile %1").arg(siteNameField.text))
                        }
                    }
                }
                Button {
                    text: qsTr("Delete")
                    Layout.fillWidth: true
                    onClicked: {
                        if (backend.deleteSite(siteNameField.text)) {
                            savedSites = backend.listSites()
                            root.addLog(qsTr("Deleted site profile %1").arg(siteNameField.text))
                        }
                    }
                }
                Button {
                    text: qsTr("Connect")
                    Layout.fillWidth: true
                    onClicked: {
                        var connected = false
                        if (authMethodCombo.currentText === "SSH Private Key") {
                            connected = remoteModel.connectKey(siteHostField.text, sitePortField.value, siteUsernameField.text, sitePrivateKeyField.text, sitePrivateKeyPassphraseField.text, siteRemotePathField.text)
                        } else {
                            connected = remoteModel.connectPassword(siteHostField.text, sitePortField.value, siteUsernameField.text, sitePasswordField.text, siteRemotePathField.text)
                        }
                        if (connected) {
                            root.addLog(qsTr("Connected to %1:%2 as %3").arg(siteHostField.text).arg(sitePortField.value).arg(siteUsernameField.text))
                            siteManagerDialog.close()
                        }
                    }
                }
            }
        }

        onOpened: {
            savedSites = backend.listSites()
            root.scannedSshKeys = backend.listSshPrivateKeys()
            if (root.scannedSshKeys.length > 0 && sitePrivateKeyField.text.length === 0) {
                sitePrivateKeyField.text = root.scannedSshKeys[0]
            }
        }
    }

    Drawer {
        id: transferQueueDrawer
        edge: Qt.RightEdge
        width: Math.min(root.width * 0.38, 460)
        height: root.height

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 16
            spacing: 12

            Label { text: qsTr("Transfer Queue"); font.pixelSize: 22; font.bold: true }
            Label {
                Layout.fillWidth: true
                text: qsTr("Queue is bound to the GUI bridge. Completed local-copy jobs execute through Rust transfer semantics; SFTP transfer buttons are exposed in the remote pane.")
                wrapMode: Text.WordWrap
            }
            ListView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                model: queueModel
                delegate: ItemDelegate { width: ListView.view.width; text: direction + ": " + source + " → " + destination + " [" + state + "]" }
                Label {
                    anchors.centerIn: parent
                    visible: queueModel.rowCount() === 0
                    text: qsTr("No active transfers")
                }
            }
            Button {
                text: qsTr("Clear Finished")
                onClicked: queueModel.clearFinished()
            }
        }
    }

    Dialog {
        id: overwritePromptDialog
        title: qsTr("Overwrite Confirmation")
        modal: true
        standardButtons: Dialog.Yes | Dialog.No | Dialog.Cancel
        anchors.centerIn: parent
        Label {
            text: qsTr("Prompt broker UI placeholder. Real overwrite/host-key prompts will route through this pattern.")
            wrapMode: Text.WordWrap
            width: 360
        }
        onAccepted: statusText = qsTr("Prompt accepted")
        onRejected: statusText = qsTr("Prompt rejected or cancelled")
    }

    Dialog {
        id: aboutDialog
        title: qsTr("About CrossSCP")
        modal: true
        standardButtons: Dialog.Close
        width: Math.min(root.width * 0.72, 520)
        height: Math.min(root.height * 0.78, 560)
        anchors.centerIn: parent
        contentItem: ScrollView {
            id: aboutScroll
            clip: true
            ScrollBar.vertical.policy: ScrollBar.AsNeeded
            ScrollBar.horizontal.policy: ScrollBar.AsNeeded
            ColumnLayout {
                width: aboutScroll.availableWidth
                spacing: 10
                Image {
                    Layout.alignment: Qt.AlignHCenter
                    source: "qrc:/qt/qml/CrossSCP/resources/icons/crossscp-256.png"
                    width: 56
                    height: 56
                    fillMode: Image.PreserveAspectFit
                }
                Label { Layout.alignment: Qt.AlignHCenter; text: qsTr("CrossSCP"); font.pixelSize: 24; font.bold: true }
                Label {
                    Layout.fillWidth: true
                text: qsTr("Creator: <a href='https://github.com/pat229988'>Pratik Patel (GitHub: pat229988)</a>")
                textFormat: Text.RichText
                onLinkActivated: function(link) { Qt.openUrlExternally(link) }
                wrapMode: Text.WordWrap
                horizontalAlignment: Text.AlignHCenter
                linkColor: "#0b6bcb"
            }
                Button {
                    Layout.alignment: Qt.AlignHCenter
                    text: qsTr("Open GitHub Repository")
                    onClicked: Qt.openUrlExternally("https://github.com/pat229988/Cross-SCP.git")
                }
                Label {
                    Layout.fillWidth: true
                    text: qsTr("Cross-platform file transfer client built with Rust and Qt.")
                    wrapMode: Text.WordWrap
                    horizontalAlignment: Text.AlignHCenter
                }
                Label {
                    Layout.fillWidth: true
                    text: qsTr("License: AGPL-3.0-or-later")
                    wrapMode: Text.WordWrap
                    horizontalAlignment: Text.AlignHCenter
                }
            }
        }
    }

    component RemotePane: Frame {
        required property string title
        required property string subtitle
        required property RemoteFileModel remoteFileModel
        property color paneAccent: "#6b46c1"

        ColumnLayout {
            anchors.fill: parent
            spacing: 8

            Rectangle { Layout.fillWidth: true; height: 3; color: paneAccent }

            Label { text: title; font.pixelSize: 18; font.bold: true }
            Label { Layout.fillWidth: true; text: subtitle; color: "#666"; elide: Text.ElideRight }

            RowLayout {
                Layout.fillWidth: true
                Button {
                    text: qsTr("Up")
                    Layout.minimumWidth: 72
                    enabled: remoteFileModel.connected
                    onClicked: remoteFileModel.goUp()
                }
                TextField {
                    Layout.fillWidth: true
                    text: remoteFileModel.path
                    selectByMouse: true
                    onAccepted: {
                        remoteFileModel.path = text
                        remoteFileModel.refresh()
                    }
                    Accessible.name: qsTr("Remote path")
                }
                Button {
                    text: qsTr("Refresh")
                    Layout.minimumWidth: 96
                    enabled: remoteFileModel.connected
                    onClicked: remoteFileModel.refresh()
                }
            }

            Label {
                Layout.fillWidth: true
                visible: remoteFileModel.error.length > 0
                text: remoteFileModel.error
                color: "#b00020"
                wrapMode: Text.WordWrap
            }

            ListView {
                id: remoteListView
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: remoteFileModel
                delegate: ItemDelegate {
                    width: ListView.view.width
                    highlighted: root.remoteItemSelected(remotePath) || ListView.isCurrentItem
                    contentItem: RowLayout {
                        spacing: 8
                        CheckBox {
                            checked: root.remoteItemSelected(remotePath)
                            onClicked: root.toggleRemoteSelection({ path: remotePath, name: name, isDirectory: isDirectory })
                            Accessible.name: qsTr("Select remote %1").arg(name)
                        }
                        Label {
                            Layout.fillWidth: true
                            text: (isDirectory ? "📁 " : "📄 ") + name + (isDirectory ? "" : "  (" + size + " bytes)")
                            elide: Text.ElideMiddle
                        }
                    }
                    onClicked: {
                        remoteListView.currentIndex = index
                        root.selectedRemotePath = remotePath
                        root.selectedRemoteName = name
                        root.selectedRemoteIsDirectory = isDirectory
                        root.transferRemotePath = remotePath
                        root.transferLocalPath = root.joinLocalPath(leftModel.path, name)
                    }
                    onDoubleClicked: remoteFileModel.openRow(index)
                    Accessible.name: (isDirectory ? qsTr("Remote folder ") : qsTr("Remote file ")) + name
                }
            }

            Label {
                Layout.fillWidth: true
                text: root.selectedRemoteItems.length > 0 ? qsTr("Selected remote items: %1").arg(root.selectedRemoteItems.length) : (root.selectedRemotePath.length > 0 ? qsTr("Selected remote: %1").arg(root.selectedRemotePath) : qsTr("Select remote files to download, or double-click folders to open them."))
                color: "#555"
                elide: Text.ElideMiddle
            }

            GridLayout {
                Layout.fillWidth: true
                columns: 2
                Label { text: qsTr("Remote file") }
                TextField {
                    Layout.fillWidth: true
                    text: root.transferRemotePath
                    placeholderText: root.selectedRemotePath.length > 0 ? root.selectedRemotePath : root.joinRemotePath(remoteFileModel.path, root.selectedLocalName.length > 0 ? root.selectedLocalName : "file.txt")
                    onTextChanged: if (text !== root.transferRemotePath) root.transferRemotePath = text
                }
            }

            GridLayout {
                Layout.fillWidth: true
                columns: 4
                ComboBox {
                    id: remoteActionCombo
                    Layout.fillWidth: true
                    model: [qsTr("Download"), qsTr("Upload")]
                }
                ActionButton {
                    text: remoteActionCombo.currentText
                    actionIcon: remoteActionCombo.currentText === qsTr("Download") ? "qrc:/qt/qml/CrossSCP/resources/actions/download.png" : "qrc:/qt/qml/CrossSCP/resources/actions/upload.png"
                    Layout.fillWidth: true
                    enabled: remoteFileModel.connected
                    onClicked: root.runPaneAction(remoteActionCombo.currentText)
                }
                ActionButton {
                    text: qsTr("New Folder")
                    actionIcon: "qrc:/qt/qml/CrossSCP/resources/actions/new-folder.png"
                    Layout.fillWidth: true
                    enabled: remoteFileModel.connected
                    onClicked: root.openNewFolderDialog(true)
                }
                ActionButton {
                    text: qsTr("Delete")
                    actionIcon: "qrc:/qt/qml/CrossSCP/resources/actions/delete.svg"
                    Layout.fillWidth: true
                    enabled: remoteFileModel.connected && (root.transferRemotePath.length > 0 || root.selectedRemotePath.length > 0)
                    onClicked: root.openDeleteDialog(true)
                }
            }
        }
    }

    component FilePane: Frame {
        required property string title
        required property string subtitle
        required property LocalFileModel fileModel
        property color paneAccent: "#0b6bcb"

        ColumnLayout {
            anchors.fill: parent
            spacing: 8

            Rectangle { Layout.fillWidth: true; height: 3; color: paneAccent }

            Label { text: title; font.pixelSize: 18; font.bold: true }
            Label { Layout.fillWidth: true; text: subtitle; color: "#666"; elide: Text.ElideRight }

            RowLayout {
                Layout.fillWidth: true
                Button { text: qsTr("Up"); Layout.minimumWidth: 72; onClicked: fileModel.goUp() }
                TextField {
                    Layout.fillWidth: true
                    text: fileModel.path
                    selectByMouse: true
                    onAccepted: fileModel.path = text
                    Accessible.name: qsTr("Path")
                }
                Button { text: qsTr("Refresh"); Layout.minimumWidth: 96; onClicked: fileModel.refresh() }
            }

            Label {
                Layout.fillWidth: true
                visible: fileModel.error.length > 0
                text: fileModel.error
                color: "#b00020"
                wrapMode: Text.WordWrap
            }

            ListView {
                id: localListView
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: fileModel
                delegate: ItemDelegate {
                    width: ListView.view.width
                    highlighted: root.localItemSelected(path) || ListView.isCurrentItem
                    contentItem: RowLayout {
                        spacing: 8
                        CheckBox {
                            checked: root.localItemSelected(path)
                            onClicked: root.toggleLocalSelection({ path: path, name: name, isDirectory: isDirectory })
                            Accessible.name: qsTr("Select local %1").arg(name)
                        }
                        Label {
                            Layout.fillWidth: true
                            text: (isDirectory ? "📁 " : "📄 ") + name + (isDirectory ? "" : "  (" + size + " bytes)")
                            elide: Text.ElideMiddle
                        }
                    }
                    onClicked: {
                        localListView.currentIndex = index
                        root.selectedLocalPath = path
                        root.selectedLocalName = name
                        root.selectedLocalIsDirectory = isDirectory
                        root.transferLocalPath = path
                        root.transferRemotePath = root.joinRemotePath(remoteModel.path, name)
                    }
                    onDoubleClicked: fileModel.openRow(index)
                    Accessible.name: (isDirectory ? qsTr("Local folder ") : qsTr("Local file ")) + name
                }
            }

            Label {
                Layout.fillWidth: true
                text: root.selectedLocalItems.length > 0 ? qsTr("Selected local items: %1").arg(root.selectedLocalItems.length) : (root.selectedLocalPath.length > 0 ? qsTr("Selected local: %1").arg(root.selectedLocalPath) : qsTr("Select local files to upload, or double-click folders to open them."))
                color: "#555"
                elide: Text.ElideMiddle
            }

            GridLayout {
                Layout.fillWidth: true
                columns: 2
                Label { text: qsTr("Local file") }
                TextField {
                    Layout.fillWidth: true
                    text: root.transferLocalPath
                    placeholderText: root.selectedLocalPath.length > 0 ? root.selectedLocalPath : qsTr("Select a local file or type a path")
                    onTextChanged: if (text !== root.transferLocalPath) root.transferLocalPath = text
                }
            }

            GridLayout {
                Layout.fillWidth: true
                columns: 4
                ComboBox {
                    id: localActionCombo
                    Layout.fillWidth: true
                    model: [qsTr("Upload"), qsTr("Download")]
                }
                ActionButton {
                    text: localActionCombo.currentText
                    actionIcon: localActionCombo.currentText === qsTr("Upload") ? "qrc:/qt/qml/CrossSCP/resources/actions/upload.png" : "qrc:/qt/qml/CrossSCP/resources/actions/download.png"
                    Layout.fillWidth: true
                    enabled: localActionCombo.currentText === qsTr("Upload") ? root.transferLocalPath.length > 0 : remoteModel.connected
                    onClicked: root.runPaneAction(localActionCombo.currentText)
                }
                ActionButton {
                    text: qsTr("New Folder")
                    actionIcon: "qrc:/qt/qml/CrossSCP/resources/actions/new-folder.png"
                    Layout.fillWidth: true
                    onClicked: root.openNewFolderDialog(false)
                }
                ActionButton {
                    text: qsTr("Delete")
                    actionIcon: "qrc:/qt/qml/CrossSCP/resources/actions/delete.svg"
                    Layout.fillWidth: true
                    enabled: root.transferLocalPath.length > 0 || root.selectedLocalPath.length > 0
                    onClicked: root.openDeleteDialog(false)
                }
            }
        }
    }

    component QueueStrip: Frame {
        ColumnLayout {
            anchors.fill: parent
            spacing: 4
            RowLayout {
                Layout.fillWidth: true
                Label { text: qsTr("Queue"); font.bold: true }
                Label {
                    Layout.fillWidth: true
                    text: qsTr("Current session transfer queue")
                    color: "#666"
                    elide: Text.ElideRight
                }
                Button {
                    text: qsTr("Clear Finished")
                    onClicked: {
                        queueModel.clearFinished()
                        root.addLog(qsTr("Cleared finished queue entries"))
                    }
                }
            }
            ListView {
                id: queueListView
                Layout.fillWidth: true
                Layout.fillHeight: true
                orientation: ListView.Horizontal
                spacing: 8
                clip: true
                model: queueModel
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.horizontal: ScrollBar { policy: ScrollBar.AsNeeded }
                ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
                delegate: Frame {
                    width: 330
                    height: ListView.view.height
                    RowLayout {
                        anchors.fill: parent
                        Label { text: direction; font.bold: true }
                        Label { Layout.fillWidth: true; text: source + " → " + destination; elide: Text.ElideMiddle }
                        Label { text: state; color: "#555" }
                    }
                }
                Label {
                    anchors.centerIn: parent
                    visible: queueModel.rowCount() === 0
                    text: qsTr("No active queued transfers")
                    color: "#777"
                }
            }
        }
    }

    component LogsPanel: Frame {
        ColumnLayout {
            anchors.fill: parent
            spacing: 4
            RowLayout {
                Layout.fillWidth: true
                Label { text: qsTr("Session Logs"); font.bold: true }
                Label {
                    Layout.fillWidth: true
                    text: qsTr("Records modifications for this app session only")
                    color: "#666"
                    elide: Text.ElideRight
                }
                Button {
                    text: qsTr("Clear Logs")
                    onClicked: sessionLogModel.clear()
                }
            }
            ListView {
                id: logListView
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: sessionLogModel
                contentWidth: Math.max(width, 1200)
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.horizontal: ScrollBar { policy: ScrollBar.AsNeeded }
                ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
                onCountChanged: if (count > 0) positionViewAtEnd()
                delegate: Label {
                    id: logText
                    width: Math.max(logListView.width, implicitWidth + 24)
                    text: timestamp + "  " + message
                    color: "#333"
                }
                Label {
                    anchors.centerIn: parent
                    visible: sessionLogModel.count === 0
                    text: qsTr("No modifications yet")
                    color: "#777"
                }
            }
        }
    }

    component ActionButton: Button {
        id: actionButton
        property string actionIcon: ""
        icon.color: "transparent"
        contentItem: RowLayout {
            spacing: 6
            Image {
                source: actionButton.actionIcon
                visible: actionButton.actionIcon.length > 0
                Layout.preferredWidth: 18
                Layout.preferredHeight: 18
                fillMode: Image.PreserveAspectFit
                opacity: actionButton.enabled ? 1.0 : 0.38
            }
            Label {
                Layout.fillWidth: true
                text: actionButton.text
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
                elide: Text.ElideRight
                color: actionButton.enabled ? actionButton.palette.buttonText : actionButton.palette.mid
            }
        }
    }
}
