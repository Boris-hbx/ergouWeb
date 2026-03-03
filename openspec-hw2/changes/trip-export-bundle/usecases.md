## Use Cases

### Use Case: 打包下载差旅报销材料

**Primary Actor:** 已登录用户
**Scope:** Next 差旅模块
**Level:** User goal

**Stakeholders and Interests:**
- 用户 — 一键获取完整报销材料（Excel + 票据），直接提交财务
- 财务人员 — 收到的材料结构清晰，每个事项的票据独立分文件夹

**Preconditions:**
- 用户已登录且有某个差旅行程的访问权限
- 行程中至少有一个条目

**Success Guarantee (Postconditions):**
- 用户下载到一个 zip 文件，包含：
  - 根目录下的 `{行程名}-报销清单.xlsx`
  - 按报销事项分组的照片文件夹，每个文件夹名为 `日期 - 描述`

**Trigger:** 用户在差旅详情页点击导出菜单中的"打包下载"

**Main Success Scenario:**
1. 用户打开差旅详情页，点击导出按钮
2. 系统显示导出菜单，包含"打包下载（Excel + 照片）"选项
3. 用户点击"打包下载"
4. 系统生成 zip：Excel 放根目录，照片按事项分文件夹
5. 浏览器下载 zip 文件，文件名为 `{行程名}-报销材料.zip`

**Extensions:**
- 4a. 行程无任何照片：zip 中只包含 Excel，无照片文件夹
- 4b. 某事项无描述：文件夹名为 `日期 - 未命名`
- 4c. 多个事项同日期同描述：文件夹名自动追加序号（如 `2026-3-1 - 午餐 (2)`）

---

### Use Case: 照片按事项分文件夹

**Primary Actor:** 系统
**Scope:** Next 后端
**Level:** Subfunction

**Stakeholders and Interests:**
- 用户 — 解压后能快速找到某个事项的所有票据

**Preconditions:**
- 行程中有带照片的条目

**Success Guarantee (Postconditions):**
- 每个 trip_item 生成一个文件夹
- 文件夹名格式: `日期 - 描述`
- 文件夹内照片按上传顺序编号

**Trigger:** 打包下载或照片下载被触发

**Main Success Scenario:**
1. 系统查询行程下所有条目及其照片
2. 按 trip_item 分组，为每个有照片的 item 创建文件夹
3. 文件夹名由 `item.date` + `item.description` 组成
4. 文件夹内照片命名为 `01.jpg`、`02.jpg`...

**Extensions:**
- 2a. 文件夹名含非法文件系统字符（如 `/`、`\`、`:`）：替换为安全字符（如 `→` 保留，`/` 替换为 `-`）
- 3a. 描述为空：使用"未命名"作为文件夹名的描述部分
