## Context

当前差旅导出分两个独立端点：
- `export_xlsx` — 生成 xlsx 并返回
- `export_photos` — 读取照片按日期分文件夹打包 zip

两者共用数据源（`trips` + `trip_items` + `trip_photos`），但互相独立。用户需要分别下载再手动整理。

## Goals / Non-Goals

**Goals:**
- 一键下载包含 Excel + 照片的完整报销材料 zip
- 照片按报销事项（trip_item）分文件夹，文件夹名含日期和描述

**Non-Goals:**
- 修改现有 `export_xlsx` 或 `export_photos` 端点（保留向后兼容）
- 照片压缩/缩放优化
- PDF 报销单生成

## Decisions

### 1. 复用 xlsx 生成逻辑，提取为公共函数

**决定**: 将 `export_xlsx` 中的 workbook 生成逻辑提取为 `fn build_xlsx_buffer(db, trip_id) -> Result<Vec<u8>, ...>`，`export_xlsx` 和 `export_bundle` 都调用它。

**理由**: 避免代码重复。xlsx 生成逻辑约 90 行，提取后两个端点共用。

### 2. 照片按 item_id 分组，而非按日期

**决定**: 以 `trip_item.id` 为分组 key，文件夹名从 `item.date` + `item.description` 生成。

**理由**: 用户要求的是"每个报销事项一个文件夹"，一个事项对应一个 `trip_item`。之前按日期分组会把同一天的不同事项混在一起。

### 3. 文件夹名安全化策略

**决定**: 保留中文、英文、数字、常见标点（`-_()→.`），将文件系统非法字符（`/\:*?"<>|`）替换为 `-`，截断到 80 字符。

**理由**: zip 内文件名需要跨平台兼容（Windows 限制最多），保留 `→` 等用户常用符号提升可读性。

### 4. zip 内部结构

```
{行程名}-报销材料.zip
├── {行程名}-报销清单.xlsx
├── 2026-3-1 - Uber Waterloo→Toronto Pearson机场/
│   ├── 01.jpg
│   └── 02.jpg
├── 2026-3-1 - 酒店/
│   └── 01.jpg
└── 2026-3-2 - 午餐/
    ├── 01.jpg
    └── 02.png
```

## Risks / Trade-offs

**[Risk] 大行程（多照片）内存占用高** → 当前 `export_photos` 已用内存 zip，bundle 同理。照片通常已压缩（JPEG），zip 压缩收益有限。如果未来遇到超大行程，可改为流式写入。

**[Risk] 文件夹名截断后可能冲突** → 通过追加序号 `(2)` 解决。
