// 把偏好设置里的外观项应用到 <html> 的 data-* 属性，驱动 CSS 变量预设。
import { Accent, Density } from "./api";

export function applyAccent(accent: Accent) {
  document.documentElement.setAttribute("data-accent", accent);
}

export function applyDensity(density: Density) {
  document.documentElement.setAttribute("data-density", density);
}

export function applyAppearance(accent: Accent, density: Density) {
  applyAccent(accent);
  applyDensity(density);
}
