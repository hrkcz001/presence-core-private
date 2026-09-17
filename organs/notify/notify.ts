#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import * as os from "node:os";
import * as process from "node:process";

function parseArgs(): Record<string, string> {
  const args: Record<string, string> = {};
  const argv = process.argv.slice(2);
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg.startsWith("--")) {
      const key = arg.slice(2);
      if (i + 1 < argv.length && !argv[i + 1].startsWith("--")) {
        args[key] = argv[++i];
      } else {
        args[key] = "true";
      }
    }
  }
  return args;
}

function sendWindowsToast(title: string, message: string): { ok: boolean; detail: string } {
  const safeTitle = title.replace(/'/g, "''");
  const safeMessage = message.replace(/'/g, "''");

  const psScript = `
[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
$template = [Windows.UI.Notifications.ToastTemplateType]::ToastText02
$xml = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent($template)
$textNodes = $xml.GetElementsByTagName('text')
$textNodes.Item(0).AppendChild($xml.CreateTextNode('${safeTitle}')) | Out-Null
$textNodes.Item(1).AppendChild($xml.CreateTextNode('${safeMessage}')) | Out-Null
$toast = [Windows.UI.Notifications.ToastNotification]::new($xml)
$appId = '{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\WindowsPowerShell\\v1.0\\powershell.exe'
try {
    [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier($appId).Show($toast)
    Write-Output "OK"
} catch {
    Write-Error $_.Exception.Message
    exit 1
}
`;

  const res = spawnSync(
    "powershell",
    ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", psScript],
    {
      encoding: "utf8",
      timeout: 10000,
      windowsHide: true,
    }
  );

  if (res.status === 0 && (res.stdout || "").includes("OK")) {
    return { ok: true, detail: "Notification displayed via Windows Toast" };
  } else {
    const err = (res.stderr || res.stdout || res.error?.message || "Unknown error").trim();
    return { ok: false, detail: `PowerShell toast failed: ${err}` };
  }
}

function sendLinuxToast(title: string, message: string, urgency: string = "normal"): { ok: boolean; detail: string } {
  const u = ["low", "normal", "critical"].includes(urgency) ? urgency : "normal";
  const res = spawnSync("notify-send", ["-u", u, "-a", "Presence", title, message], {
    encoding: "utf8",
    timeout: 10000,
  });

  if (res.status === 0) {
    return { ok: true, detail: "Notification sent via notify-send" };
  } else {
    const err = (res.stderr || res.error?.message || "notify-send failed").trim();
    return { ok: false, detail: err };
  }
}

function main(): void {
  const args = parseArgs();
  const title = args.title || "Presence";
  const message = args.message || "";
  const urgency = args.urgency || "normal";

  if (!message) {
    console.log(JSON.stringify({ status: "error", error: "message is required" }));
    process.exit(1);
  }

  const isWin = os.platform() === "win32";
  const { ok, detail } = isWin ? sendWindowsToast(title, message) : sendLinuxToast(title, message, urgency);

  const res = {
    status: ok ? "ok" : "error",
    detail,
    title,
  };

  console.log(JSON.stringify(res, null, 2));
  process.exit(ok ? 0 : 1);
}

main();