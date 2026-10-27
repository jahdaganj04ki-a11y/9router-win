import { cpSync, existsSync, rmSync } from "node:fs";
import { resolve } from "node:path";

const projectRoot = process.cwd();
const standaloneSource = resolve(projectRoot, ".next", "standalone");
const tauriResourcesApp = resolve(projectRoot, "src-tauri", "resources", "app");
const nodeExe = resolve(projectRoot, "src-tauri", "resources", "node", "node.exe");

if (!existsSync(standaloneSource)) {
  console.error("❌ .next/standalone nicht gefunden — zuerst 'npm run build' ausführen");
  process.exit(1);
}

if (!existsSync(nodeExe)) {
  console.error("❌ Node.js Binary fehlt: src-tauri/resources/node/node.exe");
  console.error("   Lade Node.js LTS v22 (win-x64) von nodejs.org herunter, entpacke und");
  console.error("   platziere node.exe unter src-tauri/resources/node/");
  process.exit(1);
}

if (existsSync(tauriResourcesApp)) rmSync(tauriResourcesApp, { recursive: true, force: true });
cpSync(standaloneSource, tauriResourcesApp, { recursive: true, force: true });
console.log(`✅ Standalone-Build kopiert nach ${tauriResourcesApp}`);
