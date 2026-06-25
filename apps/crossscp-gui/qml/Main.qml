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
    property bool logsNeedHorizontalScroll: false
    property string activeHost: ""
    property string activeProtocol: "sftp"
    property int activePort: 22
    property string activeUsername: ""
    property string activePassword: ""
    property string activePrivateKeyPath: ""
    property string activePrivateKeyPassphrase: ""
    property bool darkMode: false
    property color themeWindow: darkMode ? "#121212" : "#ffffff"
    property color themePanel: darkMode ? "#1e1e1e" : "#ffffff"
    property color themeRaised: darkMode ? "#262626" : "#f5f5f5"
    property color themeText: darkMode ? "#f5f5f5" : "#111111"
    property color themeMuted: darkMode ? "#d4d4d4" : "#333333"
    property color themeSubtle: darkMode ? "#a3a3a3" : "#666666"
    property color themeBorder: darkMode ? "#404040" : "#d0d0d0"
    property color themeButton: darkMode ? "#303030" : "#f3f3f3"
    property color themeError: darkMode ? "#fca5a5" : "#b00020"
    property color themeHeader: themePanel
    property color themeHeaderText: themeText

    color: themeWindow
    palette.window: themeWindow
    palette.windowText: themeText
    palette.base: themePanel
    palette.alternateBase: themeRaised
    palette.text: themeText
    palette.button: themeButton
    palette.buttonText: themeText
    palette.highlight: darkMode ? "#2563eb" : "#0b6bcb"
    palette.highlightedText: "#ffffff"

    function addLog(message) {
        var now = new Date()
        sessionLogModel.append({ timestamp: now.toLocaleTimeString(), message: message })
        if (message.length > 120) {
            logsNeedHorizontalScroll = true
        }
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
        selectedLocalIsDirectory = false
        transferLocalPath = ""
    }

    function clearRemoteSelection() {
        selectedRemoteItems = []
        selectedRemotePath = ""
        selectedRemoteName = ""
        selectedRemoteIsDirectory = false
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

    function defaultPortForProtocol(protocol) {
        if (protocol === "SFTP" || protocol === "SCP") return 22
        if (protocol === "FTP" || protocol === "FTPS") return 21
        if (protocol === "WebDAV" || protocol === "S3") return 443
        return 22
    }

    function protocolIsLive(protocol) {
        return protocol === "SFTP" || protocol === "SCP" || protocol === "FTP" || protocol === "FTPS"
    }

    function advancedConnectionUsesLocalTunnel() {
        return connectionModeCombo.currentIndex === 1 || connectionModeCombo.currentIndex === 2 || connectionModeCombo.currentIndex === 3
    }

    function effectiveConnectionHost() {
        if (!advancedConnectionUsesLocalTunnel()) return siteHostField.text.trim()
        return tunnelLocalHostField.text.trim().length > 0 ? tunnelLocalHostField.text.trim() : "127.0.0.1"
    }

    function effectiveConnectionPort() {
        return advancedConnectionUsesLocalTunnel() ? tunnelLocalPortField.value : sitePortField.value
    }

    function suggestedTunnelCommand() {
        var jumpUser = jumpUsernameField.text.trim().length > 0 ? jumpUsernameField.text.trim() + "@" : ""
        var jumpHost = jumpHostField.text.trim().length > 0 ? jumpHostField.text.trim() : "jump.example.com"
        var jumpPort = jumpPortField.value > 0 ? " -p " + jumpPortField.value : ""
        return "ssh -N -L " + tunnelLocalPortField.value + ":" + siteHostField.text.trim() + ":" + sitePortField.value + jumpPort + " " + jumpUser + jumpHost
    }

    function proxyJumpChain() {
        var hops = []
        for (var i = 0; i < nestedHopModel.count; i++) {
            var hop = nestedHopModel.get(i)
            var h = (hop.host || "").trim()
            if (h.length === 0) continue
            var u = (hop.user || "").trim()
            var p = hop.port > 0 && hop.port !== 22 ? ":" + hop.port : ""
            hops.push((u.length > 0 ? u + "@" : "") + h + p)
        }
        return hops.join(",")
    }

    function suggestedProxyJumpCommand() {
        var local = tunnelLocalHostField.text.trim().length > 0 ? tunnelLocalHostField.text.trim() : "127.0.0.1"
        var finalUser = finalSshUsernameField.text.trim().length > 0 ? finalSshUsernameField.text.trim() + "@" : ""
        var finalHost = finalSshHostField.text.trim().length > 0 ? finalSshHostField.text.trim() : "final.internal"
        return "ssh -N -L " + local + ":" + tunnelLocalPortField.value + ":" + siteHostField.text.trim() + ":" + sitePortField.value + " -J " + root.proxyJumpChain() + " -p " + finalSshPortField.value + " " + finalUser + finalHost
    }

    function performUpload() {
        if (!remoteModel.connected) {
            statusText = qsTr("Connect to a remote session before uploading")
            return
        }
        if (selectedLocalItems.length > 0) {
            var queuedUploads = 0
            var baseRemotePath = selectedRemoteIsDirectory && selectedRemotePath.length > 0 ? selectedRemotePath : remoteModel.path
            for (var i = 0; i < selectedLocalItems.length; i++) {
                var item = selectedLocalItems[i]
                var destination = joinRemotePath(baseRemotePath, item.name)
                if (queueModel.enqueueRemoteUpload(activeProtocol, activeHost, activePort, activeUsername, activePassword, activePrivateKeyPath, activePrivateKeyPassphrase, item.path, destination)) {
                    queuedUploads++
                    addLog(qsTr("Queued upload %1 → %2").arg(item.path).arg(destination))
                } else {
                    addLog(qsTr("Upload queue failed: %1").arg(item.path))
                }
            }
            statusText = qsTr("Queued %1 of %2 selected uploads").arg(queuedUploads).arg(selectedLocalItems.length)
            return
        }
        var localPath = root.transferLocalPath.length > 0 ? root.transferLocalPath : root.selectedLocalPath
        if (localPath.length === 0) {
            statusText = qsTr("Select a local file or folder before uploading")
            return
        }
        var remotePath = root.prepareUploadRemotePath(localPath)
        if (queueModel.enqueueRemoteUpload(activeProtocol, activeHost, activePort, activeUsername, activePassword, activePrivateKeyPath, activePrivateKeyPassphrase, localPath, remotePath)) {
            root.transferRemotePath = remotePath
            statusText = qsTr("Queued upload %1").arg(localPath)
            addLog(qsTr("Queued upload %1 → %2").arg(localPath).arg(remotePath))
        }
    }

    function performDownload() {
        if (!remoteModel.connected) {
            statusText = qsTr("Connect to a remote session before downloading")
            return
        }
        if (selectedRemoteItems.length > 0) {
            var queuedDownloads = 0
            for (var i = 0; i < selectedRemoteItems.length; i++) {
                var item = selectedRemoteItems[i]
                var destination = joinLocalPath(leftModel.path, item.name)
                if (queueModel.enqueueRemoteDownload(activeProtocol, activeHost, activePort, activeUsername, activePassword, activePrivateKeyPath, activePrivateKeyPassphrase, item.path, destination)) {
                    queuedDownloads++
                    addLog(qsTr("Queued download %1 → %2").arg(item.path).arg(destination))
                } else {
                    addLog(qsTr("Download queue failed: %1").arg(item.path))
                }
            }
            statusText = qsTr("Queued %1 of %2 selected downloads").arg(queuedDownloads).arg(selectedRemoteItems.length)
            return
        }
        var remotePath = root.transferRemotePath.length > 0 ? root.transferRemotePath : root.selectedRemotePath
        if (remotePath.length === 0) {
            statusText = qsTr("Select a remote file or folder before downloading")
            return
        }
        var localName = root.selectedRemoteName.length > 0 ? root.selectedRemoteName : root.fileNameFromPath(remotePath)
        var localPath = root.transferLocalPath.length > 0 ? root.transferLocalPath : root.joinLocalPath(leftModel.path, localName)
        if (queueModel.enqueueRemoteDownload(activeProtocol, activeHost, activePort, activeUsername, activePassword, activePrivateKeyPath, activePrivateKeyPassphrase, remotePath, localPath)) {
            root.transferLocalPath = localPath
            statusText = qsTr("Queued download %1").arg(remotePath)
            addLog(qsTr("Queued download %1 → %2").arg(remotePath).arg(localPath))
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

    AppBackend {
        id: backend
        Component.onCompleted: root.darkMode = systemDarkMode
    }
    LocalFileModel { id: leftModel }
    RemoteFileModel { id: remoteModel; Component.onCompleted: setBackend(backend) }
    TransferQueueModel { id: queueModel; Component.onCompleted: setBackend(backend) }

    Connections {
        target: queueModel
        function onTransferCompleted(direction, source, destination) {
            if (direction === "Upload") {
                remoteModel.refresh()
            } else if (direction === "Download") {
                leftModel.refresh()
            }
            root.statusText = qsTr("%1 completed: %2").arg(direction).arg(source)
            root.addLog(qsTr("%1 completed %2 → %3").arg(direction).arg(source).arg(destination))
        }
        function onTransferFailed(direction, source, destination, error) {
            root.statusText = qsTr("%1 failed: %2").arg(direction).arg(error)
            root.addLog(qsTr("%1 failed %2 → %3: %4").arg(direction).arg(source).arg(destination).arg(error))
        }
    }

    Connections {
        target: remoteModel
        function onPathChanged() {
            root.clearRemoteSelection()
        }
        function onConnectedChanged() {
            root.clearRemoteSelection()
        }
    }

    Connections {
        target: leftModel
        function onPathChanged() {
            root.clearLocalSelection()
            root.clearRemoteSelection()
        }
    }

    Connections {
        target: remoteModel
        function onPathChanged() {
            root.clearLocalSelection()
            root.clearRemoteSelection()
        }
    }

    FileDialog {
        id: privateKeyFileDialog
        title: qsTr("Select SSH private key")
        onAccepted: sitePrivateKeyField.text = backend.localPathFromUrl(String(selectedFile))
    }

    ListModel { id: nestedHopModel }

    function serializeNestedHops() {
        var lines = []
        for (var i = 0; i < nestedHopModel.count; i++) {
            var hop = nestedHopModel.get(i)
            var mode = hop.authMode === undefined ? 0 : hop.authMode
            var key = mode === 1 ? (hop.key || "") : ""
            var password = mode === 2 ? (hop.password || "") : ""
            lines.push([hop.user || "", hop.host || "", hop.port || 22, key, password].join("\t"))
        }
        return lines.join("\n")
    }

    function effectiveNestedHopSpecs() {
        return root.serializeNestedHops()
    }

    function profileDisplayText(line) {
        if (line && line.trim().charAt(0) === "{") {
            try {
                var profile = JSON.parse(line)
                return (profile.name || qsTr("Unnamed")) + "  —  " + (profile.protocol || "sftp").toUpperCase() + "  " + (profile.host || "")
            } catch (e) {
                return qsTr("Invalid saved profile")
            }
        }
        var fields = line.split("\t")
        return line.length > 0 ? fields[0] + "  —  " + fields[2] : ""
    }

    function nestedHopsAsArray() {
        var hops = []
        for (var i = 0; i < nestedHopModel.count; i++) {
            var hop = nestedHopModel.get(i)
            hops.push({
                user: hop.user || "",
                host: hop.host || "",
                port: hop.port || 22,
                authMode: hop.authMode === undefined ? 0 : hop.authMode,
                key: hop.key || ""
            })
        }
        return hops
    }

    function currentSiteConfigurationJson() {
        var profileName = siteNameField.text.trim()
        if (profileName.length === 0) {
            profileName = (siteUsernameField.text.trim().length > 0 ? siteUsernameField.text.trim() + "@" : "") + siteHostField.text.trim()
        }
        return JSON.stringify({
            schema: 1,
            name: profileName,
            protocol: protocolCombo.currentText.toLowerCase(),
            authMethod: authMethodCombo.currentText,
            host: siteHostField.text.trim(),
            port: sitePortField.value,
            username: siteUsernameField.text.trim(),
            remotePath: siteRemotePathField.text.trim().length > 0 ? siteRemotePathField.text.trim() : "/",
            credentialRef: siteCredentialRefField.text.trim(),
            privateKeyPath: sitePrivateKeyField.text.trim(),
            sshKeyTypeIndex: sshKeyTypeCombo.currentIndex,
            connectionModeIndex: connectionModeCombo.currentIndex,
            tunnelLocalHost: tunnelLocalHostField.text.trim().length > 0 ? tunnelLocalHostField.text.trim() : "127.0.0.1",
            tunnelLocalPort: tunnelLocalPortField.value,
            jumpUsername: jumpUsernameField.text.trim(),
            jumpHost: jumpHostField.text.trim(),
            jumpPort: jumpPortField.value,
            nestedHops: root.nestedHopsAsArray(),
            finalSshUsername: finalSshUsernameField.text.trim(),
            finalSshHost: finalSshHostField.text.trim(),
            finalSshPort: finalSshPortField.value,
            finalSshAuthModeIndex: finalSshAuthModeCombo.currentIndex,
            finalSshKeyPath: finalSshKeyField.text.trim()
        })
    }

    function applySiteConfigurationLine(line) {
        selectedSiteLine = line
        if (line && line.trim().charAt(0) === "{") {
            try {
                var profile = JSON.parse(line)
                siteNameField.text = profile.name || ""
                var savedProtocol = (profile.protocol || "sftp").toUpperCase()
                protocolCombo.currentIndex = Math.max(0, protocolCombo.model.indexOf(savedProtocol))
                authMethodCombo.currentIndex = authMethodCombo.model.indexOf(profile.authMethod || "Password") >= 0 ? authMethodCombo.model.indexOf(profile.authMethod || "Password") : 0
                siteHostField.text = profile.host || ""
                sitePortField.value = Number(profile.port || root.defaultPortForProtocol(protocolCombo.currentText))
                siteUsernameField.text = profile.username || ""
                siteRemotePathField.text = profile.remotePath || "/"
                siteCredentialRefField.text = profile.credentialRef || ""
                sitePrivateKeyField.text = profile.privateKeyPath || ""
                sshKeyTypeCombo.currentIndex = Number(profile.sshKeyTypeIndex || 0)
                connectionModeCombo.currentIndex = Number(profile.connectionModeIndex || 0)
                tunnelLocalHostField.text = profile.tunnelLocalHost || "127.0.0.1"
                tunnelLocalPortField.value = Number(profile.tunnelLocalPort || 2222)
                jumpUsernameField.text = profile.jumpUsername || ""
                jumpHostField.text = profile.jumpHost || ""
                jumpPortField.value = Number(profile.jumpPort || 22)
                nestedHopModel.clear()
                var hops = profile.nestedHops || []
                for (var i = 0; i < hops.length; i++) {
                    nestedHopModel.append({ user: hops[i].user || "", host: hops[i].host || "", port: Number(hops[i].port || 22), authMode: Number(hops[i].authMode || 0), key: hops[i].key || "", password: "" })
                }
                finalSshUsernameField.text = profile.finalSshUsername || ""
                finalSshHostField.text = profile.finalSshHost || ""
                finalSshPortField.value = Number(profile.finalSshPort || 22)
                finalSshAuthModeCombo.currentIndex = Number(profile.finalSshAuthModeIndex || 0)
                finalSshKeyField.text = profile.finalSshKeyPath || ""
                sitePasswordField.text = ""
                sitePrivateKeyPassphraseField.text = ""
                jumpPasswordField.text = ""
                finalSshPasswordField.text = ""
                root.addLog(qsTr("Restored saved profile %1").arg(siteNameField.text))
                return
            } catch (e) {
                root.addLog(qsTr("Saved profile parse failed: %1").arg(e))
            }
        }
        var fields = line.split("\t")
        siteNameField.text = fields[0] || ""
        var legacyProtocol = (fields[1] || "sftp").toUpperCase()
        protocolCombo.currentIndex = Math.max(0, protocolCombo.model.indexOf(legacyProtocol))
        siteHostField.text = fields[2] || ""
        sitePortField.value = Number(fields[3] || root.defaultPortForProtocol(protocolCombo.currentText))
        siteUsernameField.text = fields[4] || ""
        siteRemotePathField.text = fields[5] || "/"
        siteCredentialRefField.text = fields[6] || ""
    }

    function nestedHopChainLabel() {
        var names = []
        for (var i = 0; i < nestedHopModel.count; i++) {
            var hop = nestedHopModel.get(i)
            names.push((hop.user && hop.user.length > 0 ? hop.user + "@" : "") + hop.host + (hop.port !== 22 ? ":" + hop.port : ""))
        }
        var finalHostLabel = finalSshHostField.text.trim().length > 0 ? finalSshHostField.text.trim() : qsTr("final host not set")
        return names.length > 0 ? names.join(" → ") + " → " + finalHostLabel : qsTr("No hops added")
    }

    Dialog {
        id: nestedHopDialog
        title: qsTr("Nested SSH Hop Builder")
        modal: true
        standardButtons: Dialog.Close
        anchors.centerIn: parent
        width: Math.min(root.width * 0.82, 860)
        height: Math.min(root.height * 0.78, 620)

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 12
            spacing: 10

            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                text: qsTr("Add each SSH machine in the order you traverse it. Each hop can use agent/default auth, its own key, or its own password. Configure the final SSH host below.")
            }

            ListView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: nestedHopModel
                delegate: Frame {
                    width: ListView.view.width
                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 8
                        RowLayout {
                            Layout.fillWidth: true
                            Label { text: qsTr("🖥 %1.").arg(index + 1); Layout.preferredWidth: 44 }
                            TextField { text: user; placeholderText: qsTr("user"); Layout.preferredWidth: 110; onEditingFinished: nestedHopModel.setProperty(index, "user", text) }
                            TextField { text: host; placeholderText: qsTr("host/IP"); Layout.fillWidth: true; onEditingFinished: nestedHopModel.setProperty(index, "host", text) }
                            SpinBox { value: port; from: 1; to: 65535; editable: true; Layout.preferredWidth: 105; onValueModified: nestedHopModel.setProperty(index, "port", value) }
                            Button { text: qsTr("Remove"); onClicked: nestedHopModel.remove(index) }
                        }
                        GridLayout {
                            Layout.fillWidth: true
                            columns: 2
                            Label { text: qsTr("Auth") }
                            ComboBox {
                                id: hopAuthCombo
                                Layout.fillWidth: true
                                model: [qsTr("Agent / none"), qsTr("SSH key"), qsTr("Password")]
                                currentIndex: authMode === undefined ? 0 : authMode
                                onActivated: nestedHopModel.setProperty(index, "authMode", currentIndex)
                            }

                            Label { text: qsTr("SSH Key"); visible: hopAuthCombo.currentIndex === 1 }
                            ComboBox {
                                Layout.fillWidth: true
                                visible: hopAuthCombo.currentIndex === 1
                                model: root.scannedSshKeys
                                displayText: currentText.length > 0 ? root.fileNameFromPath(currentText) : qsTr("Scanned keys")
                                onActivated: nestedHopModel.setProperty(index, "key", currentText)
                            }

                            Label { text: qsTr("Key Path"); visible: hopAuthCombo.currentIndex === 1 }
                            TextField {
                                text: key
                                visible: hopAuthCombo.currentIndex === 1
                                placeholderText: qsTr("paste full private-key path, e.g. ~/.ssh/id_ed25519")
                                Layout.fillWidth: true
                                onEditingFinished: nestedHopModel.setProperty(index, "key", text)
                            }

                            Label { text: qsTr("Password"); visible: hopAuthCombo.currentIndex === 2 }
                            RowLayout {
                                Layout.fillWidth: true
                                visible: hopAuthCombo.currentIndex === 2
                                TextField {
                                    id: hopPasswordField
                                    text: password
                                    echoMode: TextInput.Password
                                    placeholderText: qsTr("password for this hop")
                                    Layout.fillWidth: true
                                    onEditingFinished: nestedHopModel.setProperty(index, "password", text)
                                }
                                ToolButton {
                                    text: qsTr("👁")
                                    ToolTip.visible: hovered
                                    ToolTip.text: qsTr("Hold to reveal password")
                                    onPressed: hopPasswordField.echoMode = TextInput.Normal
                                    onReleased: hopPasswordField.echoMode = TextInput.Password
                                    onCanceled: hopPasswordField.echoMode = TextInput.Password
                                }
                            }

                            Label { text: ""; visible: hopAuthCombo.currentIndex === 0 }
                            Label {
                                Layout.fillWidth: true
                                visible: hopAuthCombo.currentIndex === 0
                                color: root.themeSubtle
                                wrapMode: Text.WordWrap
                                text: qsTr("Uses ssh-agent/default SSH config. No key path or password will be passed for this hop.")
                            }
                        }
                    }
                }
            }

            Frame {
                Layout.fillWidth: true
                ColumnLayout {
                    anchors.fill: parent
                    Label { text: qsTr("🎯 Final SSH Host"); font.bold: true }
                    RowLayout {
                        Layout.fillWidth: true
                        ThemedTextField { id: finalSshUsernameField; Layout.preferredWidth: 140; placeholderText: qsTr("final user") }
                        ThemedTextField { id: finalSshHostField; Layout.fillWidth: true; placeholderText: qsTr("final.internal") }
                        SpinBox { id: finalSshPortField; from: 1; to: 65535; value: 22; editable: true }
                    }
                    GridLayout {
                        Layout.fillWidth: true
                        columns: 2
                        Label { text: qsTr("Auth") }
                        ComboBox { id: finalSshAuthModeCombo; Layout.fillWidth: true; model: [qsTr("Agent / none"), qsTr("SSH key"), qsTr("Password")] }
                        Label { text: qsTr("SSH Key"); visible: finalSshAuthModeCombo.currentIndex === 1 }
                        ComboBox { Layout.fillWidth: true; visible: finalSshAuthModeCombo.currentIndex === 1; model: root.scannedSshKeys; displayText: currentText.length > 0 ? root.fileNameFromPath(currentText) : qsTr("Scanned keys"); onActivated: finalSshKeyField.text = currentText }
                        Label { text: qsTr("Key Path"); visible: finalSshAuthModeCombo.currentIndex === 1 }
                        ThemedTextField { id: finalSshKeyField; Layout.fillWidth: true; visible: finalSshAuthModeCombo.currentIndex === 1; placeholderText: qsTr("paste final-host private-key path") }
                        Label { text: qsTr("Password"); visible: finalSshAuthModeCombo.currentIndex === 2 }
                        RowLayout {
                            Layout.fillWidth: true
                            visible: finalSshAuthModeCombo.currentIndex === 2
                            ThemedTextField { id: finalSshPasswordField; Layout.fillWidth: true; echoMode: TextInput.Password; placeholderText: qsTr("final-host password") }
                            ToolButton {
                                text: qsTr("👁")
                                ToolTip.visible: hovered
                                ToolTip.text: qsTr("Hold to reveal password")
                                onPressed: finalSshPasswordField.echoMode = TextInput.Normal
                                onReleased: finalSshPasswordField.echoMode = TextInput.Password
                                onCanceled: finalSshPasswordField.echoMode = TextInput.Password
                            }
                        }
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                Button {
                    text: qsTr("+ Add Hop")
                    onClicked: nestedHopModel.append({ user: "", host: "", port: 22, authMode: 0, key: "", password: "" })
                }
                Item { Layout.fillWidth: true }
                Button { text: qsTr("Clear"); onClicked: nestedHopModel.clear() }
            }
        }
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
            ThemedTextField {
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
                color: root.themeError
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
        background: Rectangle { color: root.themeRaised }
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
                    onClicked: Qt.openUrlExternally("https://pat229988.github.io/Cross-SCP/")
                }
            }

            Label {
                text: qsTr("CrossSCP")
                font.pixelSize: 18
                font.bold: true
                color: root.themeText
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
                    backend.stopSshTunnel()
                    root.activeHost = ""
                    root.activeUsername = ""
                    root.activePassword = ""
                    root.activePrivateKeyPath = ""
                    root.activePrivateKeyPassphrase = ""
                    root.clearRemoteSelection()
                    root.transferRemotePath = ""
                    queueModel.clearAll()
                    sessionLogModel.clear()
                    root.logsNeedHorizontalScroll = false
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

            Item {
                Layout.fillWidth: true
            }

            Switch {
                id: themeSwitch
                checked: root.darkMode
                text: checked ? qsTr("Dark") : qsTr("Light")
                onToggled: root.darkMode = checked
                Accessible.name: qsTr("Toggle light and dark mode")
            }

            Switch {
                id: openSshBackendSwitch
                checked: queueModel.useOpenSshBackend
                text: checked ? qsTr("OpenSSH") : qsTr("Rust")
                ToolTip.visible: hovered
                ToolTip.text: qsTr("Use system scp as an external child process for SFTP/SCP transfers. CrossSCP does not link or bundle OpenSSH.")
                onToggled: {
                    queueModel.useOpenSshBackend = checked
                    root.addLog(checked ? qsTr("Enabled OpenSSH transfer backend (external child process)") : qsTr("Enabled internal Rust transfer backend"))
                }
                Accessible.name: qsTr("Toggle OpenSSH transfer backend")
            }
        }
    }

    SplitView {
        anchors.fill: parent
        orientation: Qt.Vertical

        SplitView {
            SplitView.fillWidth: true
            SplitView.preferredHeight: 520
            SplitView.minimumHeight: 260
            orientation: Qt.Horizontal

            FilePane {
                SplitView.preferredWidth: 640
                SplitView.minimumWidth: 420
                title: qsTr("Local")
                subtitle: qsTr("Local filesystem")
                fileModel: leftModel
                paneAccent: "#0b6bcb"
            }

            RemotePane {
                SplitView.preferredWidth: 640
                SplitView.minimumWidth: 520
                title: remoteModel.connected ? qsTr("Remote SFTP") : qsTr("Remote SFTP")
                subtitle: remoteModel.connected ? qsTr("Connected through Rust CLI SFTP bridge") : qsTr("Open Sites, choose password or SSH key auth, then Connect")
                remoteFileModel: remoteModel
                paneAccent: "#6b46c1"
            }
        }

        SplitView {
            SplitView.fillWidth: true
            SplitView.preferredHeight: 260
            SplitView.minimumHeight: 120
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
                    text: root.profileDisplayText(modelData)
                    onClicked: root.applySiteConfigurationLine(modelData)
                }
            }

            ScrollView {
                id: siteFormScroll
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                ScrollBar.vertical.policy: ScrollBar.AsNeeded
                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

            GridLayout {
                width: siteFormScroll.availableWidth
                columns: 2

                Label { text: qsTr("Protocol") }
                ComboBox {
                    id: protocolCombo
                    Layout.fillWidth: true
                model: ["SFTP", "SCP", "FTP", "FTPS", "WebDAV", "S3"]
                onCurrentTextChanged: sitePortField.value = root.defaultPortForProtocol(currentText)
                ToolTip.visible: hovered
                ToolTip.text: qsTr("SFTP, SCP transfer-only, FTP, and explicit FTPS are live. WebDAV/S3 are selectable for profile planning and will be enabled as their adapters land.")
            }

            Label { text: ""; visible: !root.protocolIsLive(protocolCombo.currentText) }
            Label {
                Layout.fillWidth: true
                visible: !root.protocolIsLive(protocolCombo.currentText)
                color: root.themeError
                wrapMode: Text.WordWrap
                            text: qsTr("%1 adapter is planned but not implemented yet. Select SFTP, SCP, FTP, or FTPS for live connections.").arg(protocolCombo.currentText)
            }

                Label { text: qsTr("Authentication") }
                ComboBox { id: authMethodCombo; Layout.fillWidth: true; model: ["Password", "SSH Private Key"] }

                Label { text: qsTr("Profile Name") }
                ThemedTextField { id: siteNameField; Layout.fillWidth: true; placeholderText: qsTr("Production") }

                Label { text: qsTr("Host") }
                ThemedTextField { id: siteHostField; Layout.fillWidth: true; placeholderText: qsTr("sftp.example.com") }

                Label { text: qsTr("Port") }
                SpinBox {
                    id: sitePortField
                    from: 1
                    to: 65535
                    value: 22
                    editable: true
                    ToolTip.visible: hovered
                    ToolTip.text: qsTr("Type the target service port, for example 22, 2222, 21, 1204, or another custom port.")
                }

                Label { text: qsTr("Username") }
                ThemedTextField { id: siteUsernameField; Layout.fillWidth: true; placeholderText: qsTr("alice") }

                Label { text: qsTr("Remote Path") }
                ThemedTextField { id: siteRemotePathField; Layout.fillWidth: true; text: "/" }

                Label { text: qsTr("Credential Reference") }
                ThemedTextField { id: siteCredentialRefField; Layout.fillWidth: true; placeholderText: qsTr("keychain://site-name") }

                Label { text: qsTr("Advanced Connection") }
                ComboBox {
                    id: connectionModeCombo
                    Layout.fillWidth: true
                    model: [
                        qsTr("Direct host/port"),
                        qsTr("Use existing local tunnel"),
                        qsTr("SSH jump host via local tunnel"),
                        qsTr("Nested SSH hops / ProxyJump"),
                        qsTr("VPN / private network (direct after VPN)"),
                        qsTr("SOCKS/HTTP proxy (planned)")
                    ]
                    ToolTip.visible: hovered
                    ToolTip.text: qsTr("CrossSCP connects to the effective host/port. For jump hosts, create the SSH tunnel first, then connect through localhost.")
                }

                Label { text: qsTr("Tunnel Local Host"); visible: root.advancedConnectionUsesLocalTunnel() }
                ThemedTextField {
                    id: tunnelLocalHostField
                    Layout.fillWidth: true
                    visible: root.advancedConnectionUsesLocalTunnel()
                    text: "127.0.0.1"
                    placeholderText: qsTr("127.0.0.1")
                }

                Label { text: qsTr("Tunnel Local Port"); visible: root.advancedConnectionUsesLocalTunnel() }
                SpinBox {
                    id: tunnelLocalPortField
                    visible: root.advancedConnectionUsesLocalTunnel()
                    from: 1
                    to: 65535
                    value: 2222
                    editable: true
                    ToolTip.visible: hovered
                    ToolTip.text: qsTr("Type any available local port, for example 2222, 2200, or 10022.")
                }

                Label { text: qsTr("Jump Host"); visible: connectionModeCombo.currentIndex === 2 }
                RowLayout {
                    Layout.fillWidth: true
                    visible: connectionModeCombo.currentIndex === 2
                    ThemedTextField { id: jumpUsernameField; Layout.preferredWidth: 160; placeholderText: qsTr("jump user") }
                    ThemedTextField { id: jumpHostField; Layout.fillWidth: true; placeholderText: qsTr("bastion.example.com") }
                    SpinBox {
                        id: jumpPortField
                        from: 1
                        to: 65535
                        value: 22
                        editable: true
                        ToolTip.visible: hovered
                        ToolTip.text: qsTr("Type the SSH port for the jump host, usually 22 or a custom SSH port.")
                    }
                }

                Label { text: qsTr("Nested Hop Builder"); visible: connectionModeCombo.currentIndex === 3 }
                RowLayout {
                    Layout.fillWidth: true
                    visible: connectionModeCombo.currentIndex === 3
                    Label { Layout.fillWidth: true; text: root.nestedHopChainLabel(); elide: Text.ElideRight; color: root.themeSubtle }
                    Button { text: qsTr("Manage Hops…"); onClicked: nestedHopDialog.open() }
                }

                Label { text: qsTr("Jump Password"); visible: connectionModeCombo.currentIndex === 2 }
                RowLayout {
                    Layout.fillWidth: true
                    visible: connectionModeCombo.currentIndex === 2
                    ThemedTextField {
                        id: jumpPasswordField
                        Layout.fillWidth: true
                        echoMode: TextInput.Password
                        placeholderText: qsTr("Optional jump-host password; not saved")
                        ToolTip.visible: hovered
                        ToolTip.text: qsTr("Used only to answer the system ssh password prompt for the jump host. Prefer SSH agent/key auth when possible.")
                    }
                    ToolButton {
                        text: qsTr("👁")
                        ToolTip.visible: hovered
                        ToolTip.text: qsTr("Hold to reveal password")
                        onPressed: jumpPasswordField.echoMode = TextInput.Normal
                        onReleased: jumpPasswordField.echoMode = TextInput.Password
                        onCanceled: jumpPasswordField.echoMode = TextInput.Password
                    }
                }

                Label { text: ""; visible: connectionModeCombo.currentIndex !== 0 }
                Label {
                    Layout.fillWidth: true
                    visible: connectionModeCombo.currentIndex !== 0
                    color: connectionModeCombo.currentIndex === 5 ? root.themeError : root.themeSubtle
                    wrapMode: Text.WordWrap
                    text: connectionModeCombo.currentIndex === 1
                          ? qsTr("Start your tunnel first, then CrossSCP will connect to %1:%2 instead of the site host directly.").arg(tunnelLocalHostField.text).arg(tunnelLocalPortField.value)
                          : connectionModeCombo.currentIndex === 2
                            ? qsTr("CrossSCP can start this tunnel automatically with ssh. If it fails, run this command in Terminal and use Existing local tunnel:\n%1").arg(root.suggestedTunnelCommand())
                            : connectionModeCombo.currentIndex === 3
                              ? qsTr("CrossSCP will start a nested ProxyJump tunnel. Use Manage Hops for per-hop keys/passwords. Equivalent command:\n%1").arg(root.suggestedProxyJumpCommand())
                            : connectionModeCombo.currentIndex === 4
                              ? qsTr("Connect your VPN first, then use Direct host/port or leave this reminder selected.")
                              : qsTr("SOCKS/HTTP proxy is documented as a scenario but is not implemented in the backend yet.")
                }

                Label { text: qsTr("Password (not saved)"); visible: authMethodCombo.currentText === "Password" }
                RowLayout {
                    Layout.fillWidth: true
                    visible: authMethodCombo.currentText === "Password"
                    ThemedTextField {
                        id: sitePasswordField
                        Layout.fillWidth: true
                        enabled: authMethodCombo.currentText === "Password"
                        echoMode: TextInput.Password
                        placeholderText: qsTr("SFTP password for this connection")
                    }
                    ToolButton {
                        text: qsTr("👁")
                        enabled: authMethodCombo.currentText === "Password"
                        ToolTip.visible: hovered
                        ToolTip.text: qsTr("Hold to reveal password")
                        onPressed: sitePasswordField.echoMode = TextInput.Normal
                        onReleased: sitePasswordField.echoMode = TextInput.Password
                        onCanceled: sitePasswordField.echoMode = TextInput.Password
                    }
                }

                Label { text: qsTr("SSH Key Type"); visible: authMethodCombo.currentText === "SSH Private Key" }
                ComboBox {
                    id: sshKeyTypeCombo
                    Layout.fillWidth: true
                    visible: authMethodCombo.currentText === "SSH Private Key"
                    enabled: authMethodCombo.currentText === "SSH Private Key"
                    model: [
                        qsTr("Auto-detect from key file"),
                        qsTr("Ed25519"),
                        qsTr("RSA"),
                        qsTr("ECDSA P-256"),
                        qsTr("ECDSA P-384"),
                        qsTr("ECDSA P-521"),
                        qsTr("DSA / DSS (legacy)"),
                        qsTr("FIDO/U2F Ed25519-SK (backend-dependent)"),
                        qsTr("FIDO/U2F ECDSA-SK (backend-dependent)")
                    ]
                    ToolTip.visible: hovered
                    ToolTip.text: qsTr("The ssh2/libssh2 backend auto-detects the private-key file format. Hardware-backed FIDO/U2F keys may require extra backend support and can fail even when listed.")
                }

                Label { text: qsTr("Private Key"); visible: authMethodCombo.currentText === "SSH Private Key" }
                RowLayout {
                    Layout.fillWidth: true
                    visible: authMethodCombo.currentText === "SSH Private Key"
                    ComboBox {
                        id: sshKeyCombo
                        Layout.preferredWidth: 250
                        enabled: authMethodCombo.currentText === "SSH Private Key"
                        model: root.scannedSshKeys
                        displayText: currentText.length > 0 ? root.fileNameFromPath(currentText) : qsTr("Scanned ~/.ssh keys")
                        onActivated: sitePrivateKeyField.text = currentText
                    }
                    ThemedTextField {
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

                Label { text: qsTr("Key Passphrase"); visible: authMethodCombo.currentText === "SSH Private Key" }
                RowLayout {
                    Layout.fillWidth: true
                    visible: authMethodCombo.currentText === "SSH Private Key"
                    ThemedTextField {
                        id: sitePrivateKeyPassphraseField
                        Layout.fillWidth: true
                        enabled: authMethodCombo.currentText === "SSH Private Key"
                        echoMode: TextInput.Password
                        placeholderText: qsTr("Optional private key passphrase")
                    }
                    ToolButton {
                        text: qsTr("👁")
                        enabled: authMethodCombo.currentText === "SSH Private Key"
                        ToolTip.visible: hovered
                        ToolTip.text: qsTr("Hold to reveal passphrase")
                        onPressed: sitePrivateKeyPassphraseField.echoMode = TextInput.Normal
                        onReleased: sitePrivateKeyPassphraseField.echoMode = TextInput.Password
                        onCanceled: sitePrivateKeyPassphraseField.echoMode = TextInput.Password
                    }
                }

                Label { text: ""; visible: authMethodCombo.currentText === "SSH Private Key" }
                Label {
                    Layout.fillWidth: true
                    visible: authMethodCombo.currentText === "SSH Private Key"
                    color: root.themeSubtle
                    wrapMode: Text.WordWrap
                    text: qsTr("Note: changing keys currently runs the remote connection/list command synchronously, so an unreachable host, wrong passphrase, or unsupported key type can make the window appear stuck until the CLI timeout returns. The next UX fix is to move connect/list into an async QProcess like transfers.")
                }
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
                        if (backend.saveSiteConfiguration(root.currentSiteConfigurationJson())) {
                            savedSites = backend.listSites()
                            root.addLog(qsTr("Saved full site profile %1").arg(siteNameField.text.length > 0 ? siteNameField.text : siteHostField.text))
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
                        if (!root.protocolIsLive(protocolCombo.currentText)) {
                            statusText = qsTr("%1 is planned but not live yet. Use SFTP, SCP, FTP, or FTPS for now.").arg(protocolCombo.currentText)
                            return
                        }
                        if (protocolCombo.currentText !== "SFTP" && protocolCombo.currentText !== "SCP" && authMethodCombo.currentText === "SSH Private Key") {
                            statusText = qsTr("%1 supports password authentication in this release. Select Password.").arg(protocolCombo.currentText)
                            return
                        }
                        if (connectionModeCombo.currentIndex === 5) {
                            statusText = qsTr("SOCKS/HTTP proxy mode is planned but not implemented yet. Use a local tunnel or VPN workaround.")
                            return
                        }
                        var connectHost = root.effectiveConnectionHost()
                        var connectPort = root.effectiveConnectionPort()
                        if (connectionModeCombo.currentIndex === 2) {
                            if (!backend.startSshTunnel(siteHostField.text, sitePortField.value, tunnelLocalHostField.text, tunnelLocalPortField.value, jumpUsernameField.text, jumpHostField.text, jumpPortField.value, authMethodCombo.currentText === "SSH Private Key" ? sitePrivateKeyField.text : "", jumpPasswordField.text)) {
                                statusText = backend.status
                                root.addLog(qsTr("SSH jump-host tunnel failed: %1").arg(backend.status))
                                return
                            }
                            root.addLog(qsTr("Started SSH jump-host tunnel to %1:%2 via %3:%4").arg(siteHostField.text).arg(sitePortField.value).arg(jumpHostField.text).arg(jumpPortField.value))
                        }
                        if (connectionModeCombo.currentIndex === 3) {
                            var hopSpecs = root.effectiveNestedHopSpecs()
                            if (hopSpecs.length === 0 || finalSshHostField.text.trim().length === 0) {
                                statusText = qsTr("Nested SSH hops require at least Hop 1 and Final SSH Host.")
                                return
                            }
                            var finalKey = finalSshAuthModeCombo.currentIndex === 1 ? finalSshKeyField.text : ""
                            var finalPassword = finalSshAuthModeCombo.currentIndex === 2 ? finalSshPasswordField.text : ""
                            if (!backend.startManagedNestedTunnel(siteHostField.text, sitePortField.value, tunnelLocalHostField.text, tunnelLocalPortField.value, hopSpecs, finalSshUsernameField.text, finalSshHostField.text, finalSshPortField.value, finalKey, finalPassword)) {
                                statusText = backend.status
                                root.addLog(qsTr("Nested SSH tunnel failed: %1").arg(backend.status))
                                return
                            }
                            root.addLog(qsTr("Started managed nested SSH tunnel to %1:%2 via %3 hop(s)").arg(siteHostField.text).arg(sitePortField.value).arg(nestedHopModel.count > 0 ? nestedHopModel.count : 1))
                        }
                        if (authMethodCombo.currentText === "SSH Private Key") {
                            connected = remoteModel.connectKey(protocolCombo.currentText.toLowerCase(), connectHost, connectPort, siteUsernameField.text, sitePrivateKeyField.text, sitePrivateKeyPassphraseField.text, siteRemotePathField.text)
                        } else {
                            connected = remoteModel.connectPassword(protocolCombo.currentText.toLowerCase(), connectHost, connectPort, siteUsernameField.text, sitePasswordField.text, siteRemotePathField.text)
                        }
                        if (connected) {
                            root.activeProtocol = protocolCombo.currentText.toLowerCase()
                            root.activeHost = connectHost
                            root.activePort = connectPort
                            root.activeUsername = siteUsernameField.text.trim()
                            root.activePassword = authMethodCombo.currentText === "Password" ? sitePasswordField.text : ""
                            root.activePrivateKeyPath = authMethodCombo.currentText === "SSH Private Key" ? sitePrivateKeyField.text.trim() : ""
                            root.activePrivateKeyPassphrase = authMethodCombo.currentText === "SSH Private Key" ? sitePrivateKeyPassphraseField.text : ""
                            if (backend.saveSiteConfiguration(root.currentSiteConfigurationJson())) {
                                savedSites = backend.listSites()
                                root.addLog(qsTr("Updated saved profile %1").arg(siteNameField.text.length > 0 ? siteNameField.text : siteHostField.text))
                            }
                            root.addLog(qsTr("Connected to %1:%2 as %3").arg(connectHost).arg(connectPort).arg(siteUsernameField.text))
                            siteManagerDialog.close()
                        } else if (connectionModeCombo.currentIndex === 2 || connectionModeCombo.currentIndex === 3) {
                            backend.stopSshTunnel()
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
                clip: true
                contentWidth: Math.max(width, 980)
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.horizontal: ScrollBar { policy: ScrollBar.AsNeeded }
                ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
                delegate: Frame {
                    width: Math.max(ListView.view.width, 980)
                    height: 72
                    RowLayout {
                        anchors.fill: parent
                        spacing: 10
                        Label { Layout.preferredWidth: 90; text: direction; font.bold: true; elide: Text.ElideRight }
                        Label { Layout.preferredWidth: 260; text: source; elide: Text.ElideMiddle }
                        Label { Layout.preferredWidth: 260; text: destination; elide: Text.ElideMiddle }
                        ColumnLayout {
                            Layout.fillWidth: true
                            RowLayout {
                                Layout.fillWidth: true
                                ProgressBar { Layout.fillWidth: true; from: 0; to: 100; value: progress; indeterminate: state.indexOf("Running") === 0 && progress === 0 && bytesTotal === 0 }
                                Label { Layout.preferredWidth: 118; text: progress + "% · " + speedText; horizontalAlignment: Text.AlignRight; color: root.themeMuted }
                            }
                            Label { Layout.fillWidth: true; text: error.length > 0 ? state + " — " + error : state; color: error.length > 0 ? "#b00020" : "#555"; elide: Text.ElideRight }
                        }
                    }
                }
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
                    source: "qrc:/qt/qml/CrossSCP/resources/icons/crossscp-1024.png"
                    sourceSize.width: 224
                    sourceSize.height: 224
                    width: 56
                    height: 56
                    fillMode: Image.PreserveAspectFit
                    smooth: true
                    mipmap: true
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
                    text: qsTr("Open CrossSCP Website")
                    onClicked: Qt.openUrlExternally("https://pat229988.github.io/Cross-SCP/")
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
            Label { Layout.fillWidth: true; text: subtitle; color: root.themeSubtle; elide: Text.ElideRight }

            RowLayout {
                Layout.fillWidth: true
                Button {
                    text: qsTr("Up")
                    Layout.minimumWidth: 72
                    enabled: remoteFileModel.connected
                    onClicked: remoteFileModel.goUp()
                }
                ThemedTextField {
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
                color: root.themeError
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
                color: root.themeMuted
                elide: Text.ElideMiddle
            }

            GridLayout {
                Layout.fillWidth: true
                columns: 2
                Label { text: qsTr("Remote file") }
                ThemedTextField {
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
            Label { Layout.fillWidth: true; text: subtitle; color: root.themeSubtle; elide: Text.ElideRight }

            RowLayout {
                Layout.fillWidth: true
                Button { text: qsTr("Up"); Layout.minimumWidth: 72; onClicked: fileModel.goUp() }
                ThemedTextField {
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
                color: root.themeError
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
                color: root.themeMuted
                elide: Text.ElideMiddle
            }

            GridLayout {
                Layout.fillWidth: true
                columns: 2
                Label { text: qsTr("Local file") }
                ThemedTextField {
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
                    color: root.themeSubtle
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
                property int directionColumnWidth: 120
                property int fromColumnWidth: Math.max(260, Math.floor((width - directionColumnWidth - statusColumnWidth - 44) / 2))
                property int toColumnWidth: fromColumnWidth
                property int statusColumnWidth: 360
                property int tableWidth: Math.max(width, directionColumnWidth + fromColumnWidth + toColumnWidth + statusColumnWidth + 44)
                spacing: 4
                clip: true
                model: queueModel
                contentWidth: tableWidth
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.horizontal: ScrollBar { policy: ScrollBar.AsNeeded }
                ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
                header: Rectangle {
                    width: queueListView.tableWidth
                    height: 28
                    color: root.themeHeader
                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 8
                        anchors.rightMargin: 8
                        spacing: 10
                        Label { Layout.preferredWidth: queueListView.directionColumnWidth; text: qsTr("Type"); font.bold: true; color: root.themeHeaderText }
                        Label { Layout.preferredWidth: queueListView.fromColumnWidth; text: qsTr("From"); font.bold: true; color: root.themeHeaderText }
                        Label { Layout.preferredWidth: queueListView.toColumnWidth; text: qsTr("To"); font.bold: true; color: root.themeHeaderText }
                        Label { Layout.preferredWidth: queueListView.statusColumnWidth; text: qsTr("Status / Progress"); font.bold: true; color: root.themeHeaderText }
                    }
                }
                delegate: Frame {
                    width: queueListView.tableWidth
                    height: 62
                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 8
                        anchors.rightMargin: 8
                        spacing: 10
                        Label {
                            Layout.preferredWidth: queueListView.directionColumnWidth
                            text: direction
                            font.bold: true
                            elide: Text.ElideRight
                        }
                        Label {
                            Layout.preferredWidth: queueListView.fromColumnWidth
                            text: source
                            elide: Text.ElideMiddle
                        }
                        Label {
                            Layout.preferredWidth: queueListView.toColumnWidth
                            text: destination
                            elide: Text.ElideMiddle
                        }
                        ColumnLayout {
                            Layout.preferredWidth: queueListView.statusColumnWidth
                            spacing: 2
                            RowLayout {
                                Layout.fillWidth: true
                                ProgressBar {
                                    Layout.fillWidth: true
                                    from: 0
                                    to: 100
                                    value: progress
                                    indeterminate: state.indexOf("Running") === 0 && progress === 0 && bytesTotal === 0
                                }
                                Label {
                                    Layout.preferredWidth: 118
                                    text: progress + "% · " + speedText
                                    horizontalAlignment: Text.AlignRight
                                    color: root.themeMuted
                                }
                            }
                            Label {
                                Layout.fillWidth: true
                                text: error.length > 0 ? state + " — " + error : state
                                color: error.length > 0 ? root.themeError : root.themeMuted
                                elide: Text.ElideRight
                            }
                        }
                    }
                }
                Label {
                    anchors.centerIn: parent
                    visible: queueModel.rowCount() === 0 && queueListView.count === 0
                    z: 10
                    text: qsTr("No active queued transfers")
                    color: root.themeSubtle
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
                    color: root.themeSubtle
                    elide: Text.ElideRight
                }
                Button {
                    text: qsTr("Clear Logs")
                    onClicked: {
                        sessionLogModel.clear()
                        root.logsNeedHorizontalScroll = false
                    }
                }
            }
            ListView {
                id: logListView
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: sessionLogModel
                contentWidth: root.logsNeedHorizontalScroll ? Math.max(width, 1200) : width
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.horizontal: ScrollBar { policy: ScrollBar.AsNeeded }
                ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
                onCountChanged: if (count > 0) positionViewAtEnd()
                delegate: Label {
                    id: logText
                    width: logListView.contentWidth
                    text: timestamp + "  " + message
                    color: root.themeText
                    elide: root.logsNeedHorizontalScroll ? Text.ElideNone : Text.ElideRight
                }
                Label {
                    anchors.centerIn: parent
                    visible: sessionLogModel.count === 0
                    text: qsTr("No modifications yet")
                    color: root.themeSubtle
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

    component ThemedTextField: TextField {
        color: root.themeText
        placeholderTextColor: root.themeSubtle
        selectedTextColor: "#ffffff"
        selectionColor: root.palette.highlight
        background: Rectangle {
            color: root.themePanel
            border.color: root.themeBorder
            border.width: 1
            radius: 4
        }
    }
}
