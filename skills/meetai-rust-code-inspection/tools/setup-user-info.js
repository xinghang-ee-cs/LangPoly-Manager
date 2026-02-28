#!/usr/bin/env node

/*
 * User info setup for Rust code inspection workflow.
 * Writes me.config.json in this skill directory with:
 * {
 *   "date": "YYYY-MM-DD",
 *   "datetime": "YYYY-MM-DD HH:mm:ss",
 *   "name": "your-name"
 * }
 */

const fs = require("fs");
const path = require("path");
const readline = require("readline");
const { execSync } = require("child_process");

const configPath = path.join(__dirname, "..", "me.config.json");

function currentDate() {
  const now = new Date();
  const y = now.getFullYear();
  const m = String(now.getMonth() + 1).padStart(2, "0");
  const d = String(now.getDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}

function currentDateTime() {
  const now = new Date();
  const y = now.getFullYear();
  const m = String(now.getMonth() + 1).padStart(2, "0");
  const d = String(now.getDate()).padStart(2, "0");
  const hh = String(now.getHours()).padStart(2, "0");
  const mm = String(now.getMinutes()).padStart(2, "0");
  const ss = String(now.getSeconds()).padStart(2, "0");
  return `${y}-${m}-${d} ${hh}:${mm}:${ss}`;
}

function readConfig() {
  try {
    if (!fs.existsSync(configPath)) return null;
    return JSON.parse(fs.readFileSync(configPath, "utf8"));
  } catch (err) {
    console.error("读取配置文件时遇到了点麻烦：", err.message);
    return null;
  }
}

function writeConfig(config) {
  fs.writeFileSync(configPath, JSON.stringify(config, null, 2), "utf8");
}

function askName() {
  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  });

  return new Promise((resolve) => {
    rl.question("请输入您的姓名或昵称：", (name) => {
      rl.close();
      resolve(name.trim());
    });
  });
}

function parseNameArg(argv) {
  for (let i = 0; i < argv.length; i += 1) {
    const token = argv[i];
    if (token === "--name") {
      return (argv[i + 1] || "").trim();
    }
    if (token.startsWith("--name=")) {
      return token.slice("--name=".length).trim();
    }
  }
  return "";
}

function detectNameFromGit() {
  try {
    const output = execSync("git config user.name", {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    });
    const name = (output || "").trim();
    return name || "";
  } catch (_) {
    return "";
  }
}

function detectNameFromEnv() {
  const candidates = [
    process.env.CODEX_USER_NAME,
    process.env.GIT_AUTHOR_NAME,
    process.env.USERNAME,
    process.env.USER,
  ];
  for (const value of candidates) {
    const name = (value || "").trim();
    if (name) return name;
  }
  return "";
}

async function main() {
  const today = currentDate();
  const now = currentDateTime();
  const cfg = readConfig();
  const nameArg = parseNameArg(process.argv.slice(2));

  if (cfg && cfg.date === today && cfg.name && cfg.datetime) {
    console.log("您的配置已是最新状态，无需重复设置。");
    console.log(JSON.stringify(cfg, null, 2));
    return;
  }

  const autoName = nameArg || (cfg && cfg.name) || detectNameFromGit() || detectNameFromEnv();
  if (cfg && cfg.date === today && cfg.name && !cfg.datetime) {
    const upgraded = { date: today, datetime: now, name: autoName || cfg.name };
    writeConfig(upgraded);
    console.log("配置已成功升级，新增了精确时间信息：");
    console.log(JSON.stringify(upgraded, null, 2));
    return;
  }

  let name = autoName;
  if (!name && process.stdin.isTTY) {
    name = await askName();
  }

  if (!name) {
    console.error(
      "姓名不能为空哦～请通过 --name <昵称> 参数指定，或设置 USERNAME/USER 环境变量。"
    );
    process.exit(1);
  }

  const next = { date: today, datetime: now, name };
  writeConfig(next);
  console.log("太好了！配置已保存成功，欢迎您，" + name + "！");
  console.log(JSON.stringify(next, null, 2));
}

if (require.main === module) {
  main().catch((err) => {
    console.error("初始化过程中出现了错误，请检查后重试：", err.message);
    process.exit(1);
  });
}

module.exports = {
  currentDate,
  currentDateTime,
  readConfig,
};
