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
    static let stopRequestedKey = "soundlink.stop_requested"
    static let deviceIdKey = "soundlink.device_id"
    /// App Group 共享键名：是否转储采集 PCM/Opus（供 Extension 读取）。
    static let dumpPcmKey = "soundlink.dump_pcm"
    /// Broadcast Extension 的 Bundle Identifier（在 Xcode 中配置）。
    static let preferredExtension = "com.soundlink.soundlink.BroadcastExtension"

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
            guard let defaults = UserDefaults(suiteName: Self.appGroupId) else {
                result(FlutterError(code: "APP_GROUP", message: "无法访问 App Group，请检查 Runner 与 BroadcastExtension 的 App Groups 配置", details: nil))
                return
            }
            defaults.set(config, forKey: Self.configKey)
            defaults.set(false, forKey: Self.stopRequestedKey)
            result(true)

        case "startCapture":
            // iOS 无法自动开启广播，呈现系统广播选择器供用户点击。
            presentBroadcastPicker()
            result(true)

        case "stopCapture":
            let args = call.arguments as? [String: Any]
            let clearSession = args?["clearSession"] as? Bool ?? true
            let defaults = UserDefaults(suiteName: Self.appGroupId)
            defaults?.set(true, forKey: Self.stopRequestedKey)
            if clearSession {
                defaults?.removeObject(forKey: Self.configKey)
            }
            result(true)

        case "popStopReason":
            // 读取并清除 Broadcast Extension 写入的停止原因。
            let defaults = UserDefaults(suiteName: Self.appGroupId)
            let reason = defaults?.string(forKey: "soundlink.stop_reason")
            let ts = defaults?.double(forKey: "soundlink.stop_reason_ts") ?? 0
            if reason != nil {
                defaults?.removeObject(forKey: "soundlink.stop_reason")
                defaults?.removeObject(forKey: "soundlink.stop_reason_ts")
            }
            result(reason != nil ? ["reason": reason!, "ts": ts] : nil)

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
    ///
    /// RPSystemBroadcastPickerView 内部包含一个系统私有的 UIButton，点击后 iOS 会
    /// 弹出原生广播确认 UI。之前的实现把 picker 包在自定义 modal VC 中用 Auto Layout
    /// 呈现，但 picker 在被添加到 window 之前不会初始化内部按钮，且 formSheet 的布局
    /// 时序导致按钮不渲染（弹窗全白）。
    ///
    /// 修复：创建 picker 后直接触发其内部按钮的 touchUpInside，跳过自定义 UI，
    /// 让系统直接弹出广播确认对话框。
    private func presentBroadcastPicker() {
        DispatchQueue.main.async { [weak self] in
            guard self != nil else { return }
            let picker = RPSystemBroadcastPickerView(frame: .zero)
            picker.preferredExtension = Self.preferredExtension
            picker.showsMicrophoneButton = false
            // 直接触发 picker 内部按钮，弹出系统广播确认 UI。
            if let button = picker.subviews.compactMap({ $0 as? UIButton }).first {
                button.sendActions(for: .touchUpInside)
            }
        }
    }
}
