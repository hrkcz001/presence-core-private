#!/usr/bin/env python3
import sys
import os
import argparse
import subprocess
import json

def send_windows_toast(title: str, message: str, urgency: str = "normal") -> tuple[bool, str]:
    # Use PowerShell with native WinRT ToastNotificationManager
    ps_script = f"""
[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
$template = [Windows.UI.Notifications.ToastTemplateType]::ToastText02
$xml = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent($template)
$textNodes = $xml.GetElementsByTagName('text')
$textNodes.Item(0).AppendChild($xml.CreateTextNode(@'
{title}
'@.Trim())) | Out-Null
$textNodes.Item(1).AppendChild($xml.CreateTextNode(@'
{message}
'@.Trim())) | Out-Null
$toast = [Windows.UI.Notifications.ToastNotification]::new($xml)
$appId = '{{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}}\\WindowsPowerShell\\v1.0\\powershell.exe'
try {{
    [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier($appId).Show($toast)
    Write-Output "OK"
}} catch {{
    Write-Error $_.Exception.Message
    exit 1
}}
"""
    try:
        proc = subprocess.run(
            ["powershell", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", ps_script],
            capture_output=True,
            text=True,
            timeout=10,
            encoding="utf-8",
            errors="replace"
        )
        if proc.returncode == 0 and "OK" in proc.stdout:
            return True, "Notification displayed via Windows Toast"
        else:
            err = proc.stderr.strip() or proc.stdout.strip()
            return False, f"PowerShell toast failed: {err}"
    except Exception as e:
        return False, f"Exception spawning PowerShell: {e}"

def send_linux_toast(title: str, message: str, urgency: str = "normal") -> tuple[bool, str]:
    urgency_map = {"low": "low", "normal": "normal", "critical": "critical"}
    u = urgency_map.get(urgency, "normal")
    try:
        proc = subprocess.run(
            ["notify-send", "-u", u, "-a", "Presence", title, message],
            capture_output=True,
            text=True,
            timeout=10,
            encoding="utf-8",
            errors="replace"
        )
        if proc.returncode == 0:
            return True, "Notification sent via notify-send"
        else:
            return False, f"notify-send failed: {proc.stderr.strip()}"
    except FileNotFoundError:
        return False, "notify-send executable not found"
    except Exception as e:
        return False, f"Exception running notify-send: {e}"

def main():
    parser = argparse.ArgumentParser(description="Desktop notification tool for Presence")
    parser.add_argument("--title", required=True, help="Notification title")
    parser.add_argument("--message", required=True, help="Notification body message")
    parser.add_argument("--urgency", default="normal", choices=["low", "normal", "critical"], help="Urgency level")
    parser.add_argument("--action", default="toast", help="Optional action parameter")

    args = parser.parse_args()

    if sys.platform == "win32":
        ok, detail = send_windows_toast(args.title, args.message, args.urgency)
    else:
        ok, detail = send_linux_toast(args.title, args.message, args.urgency)

    res = {
        "status": "ok" if ok else "error",
        "detail": detail,
        "title": args.title
    }
    print(json.dumps(res, ensure_ascii=False))
    sys.exit(0 if ok else 1)

if __name__ == "__main__":
    main()
