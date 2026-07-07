// SoundLinkPlugin.swift
//
// Flutter MethodChannel("com.soundlink/platform") 的 iOS 实现。
// 职责：写会话配置到 App Group（供 BroadcastExtension 读取）、引导用户开启广播、
//       读取设备 ID。音频采集不在主 App 进行，由 Extension 独立完成。
//
// 集成：将本文件加入 Runner target；在 AppDelegate 注册 SoundLinkPlugin。
// 依赖：App Group "group.com.soundlink"；Runner.entitlements 已声明。

import Flutter
import UIKit
import ReplayKit

class SoundLinkPlugin: NSObject, FlutterPlugin {
    static let channelName = "com.soundlink/platform"
    static let appGroupId = "group.com.soundlink"
    static let configKey = "soundlink.session.config"
    static let deviceIdKey = "soundlink.device_id"
    /// App Group 共享键名：是否转储采集 PCM/Opus（供 Extension 读取）。
    static let dumpPcmKey = "soundlink.dump_pcm"
    /// Broadcast Extension 的 Bundle Identifier（在 Xcode 中配置）。
    static let preferredExtension = "com.soundlink.soundlink.BroadcastExtension"

    /// 当前呈现的广播选择器（强引用以保持存活）。
    private var presentedNav: UINavigationController?

    static func register(with registrar: FlutterPluginRegistrar) {
        let channel = FlutterMethodChannel(name: channelName, binaryMessenger: registrar.messenger())
        registrar.addMethodCallDelegate(SoundLinkPlugin(), channel: channel)
    }

    func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
        switch call.method {
        case "writeSessionConfig":
            guard let args = call.arguments as? [String: Any],
                  let config = args["config"] as? String else {
                result(FlutterError(code: "ARG", message: "缺少 config", details: nil))
                return
            }
            UserDefaults(suiteName: Self.appGroupId)?.set(config, forKey: Self.configKey)
            result(true)

        case "startCapture":
            // iOS 无法自动开启广播，呈现系统广播选择器供用户点击。
            presentBroadcastPicker()
            result(true)

        case "stopCapture":
            // 清除配置；广播由用户经红色状态栏停止，Extension 随后收到 broadcastFinished。
            UserDefaults(suiteName: Self.appGroupId)?.removeObject(forKey: Self.configKey)
            presentedNav?.dismiss(animated: true) { self.presentedNav = nil }
            result(true)

        case "requestMediaProjection":
            // iOS 无此概念；返回 true 保持通道语义一致。
            result(true)

        case "getDeviceId":
            result(getDeviceId())

        case "setDumpPcm":
            // 把转储开关写入 App Group，BroadcastExtension 启动时读取。
            guard let args = call.arguments as? [String: Any],
                  let enabled = args["enabled"] as? Bool else {
                result(FlutterError(code: "ARG", message: "缺少 enabled", details: nil))
                return
            }
            UserDefaults(suiteName: Self.appGroupId)?.set(enabled, forKey: Self.dumpPcmKey)
            result(true)

        default:
            result(FlutterMethodNotImplemented)
        }
    }

    /// 读取或生成持久化设备 ID。
    private func getDeviceId() -> String {
        let defaults = UserDefaults.standard
        if let id = defaults.string(forKey: Self.deviceIdKey) {
            return id
        }
        let id = "ios-" + UUID().uuidString.prefix(8).lowercased()
        defaults.set(String(id), forKey: Self.deviceIdKey)
        return String(id)
    }

    /// 呈现 RPSystemBroadcastPickerView，引导用户选择本 App 的 Extension 并开始广播。
    private func presentBroadcastPicker() {
        DispatchQueue.main.async { [weak self] in
            guard let self = self,
                  let root = UIApplication.shared.windows.first(where: { $0.isKeyWindow })?.rootViewController else {
                return
            }
            let picker = RPSystemBroadcastPickerView(frame: .zero)
            picker.preferredExtension = Self.preferredExtension
            picker.showsMicrophoneButton = false
            picker.translatesAutoresizingMaskIntoConstraints = false

            let host = UIViewController()
            host.view.backgroundColor = .systemBackground
            host.navigationItem.title = "开始广播"
            host.navigationItem.leftBarButtonItem = UIBarButtonItem(
                barButtonSystemItem: .cancel, target: self, action: #selector(self.dismissPresented))
            host.view.addSubview(picker)
            NSLayoutConstraint.activate([
                picker.centerXAnchor.constraint(equalTo: host.view.centerXAnchor),
                picker.centerYAnchor.constraint(equalTo: host.view.centerYAnchor),
                picker.widthAnchor.constraint(equalToConstant: 240),
                picker.heightAnchor.constraint(equalToConstant: 60),
            ])

            let nav = UINavigationController(rootViewController: host)
            nav.modalPresentationStyle = .formSheet
            self.presentedNav = nav
            root.present(nav, animated: true)
        }
    }

    @objc private func dismissPresented() {
        presentedNav?.dismiss(animated: true) { self.presentedNav = nil }
    }
}
