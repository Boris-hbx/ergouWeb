## Use Cases

### UC-1: 新建记账条目

**Primary Actor:** 用户
**Scope:** 记账模块
**Level:** User goal

**Preconditions:**
- 用户已登录

**Success Guarantee (Postconditions):**
- 记账条目已创建，包含金额、日期等必填信息
- 若有照片，照片已上传并关联到条目

**Trigger:** 用户点击新建记账

**Main Success Scenario:**
1. 用户打开新建记账表单
2. 用户填写金额、日期、备注等信息
3. 用户点击"保存"
4. 系统创建条目并关闭表单

**Extensions:**
- 1a. 用户添加照片：
  1. 系统通过 PhotoManager 自动压缩并显示缩略图预览
  2. 系统同时显示"保存"和"识别账单 ✨"两个按钮
  3. 用户可继续手动填写后点击"保存"（不触发 AI）
  4. 或用户点击"识别账单 ✨"，进入 AI 识别流程（→ UC-3）
- 1a-1a. 用户添加多张照片：PhotoManager 累加到列表，不覆盖之前的
- 1a-1b. 用户删除某张照片：PhotoManager 移除并更新预览；若所有照片删完，恢复为仅"保存"按钮
- 1a-1c. 文件无法解码（非图片）：PhotoManager 提示并跳过，其余正常

---

### UC-2: 编辑已有记账条目

**Primary Actor:** 用户
**Scope:** 记账模块
**Level:** User goal

**Preconditions:**
- 用户已登录，条目已存在

**Success Guarantee (Postconditions):**
- 条目信息已更新
- 若补充了新照片，照片已上传并关联

**Trigger:** 用户在详情页点击编辑

**Main Success Scenario:**
1. 系统加载条目数据到编辑表单，展示已有照片
2. 用户修改金额、日期、备注等字段
3. 用户点击"保存"
4. 系统更新条目并关闭表单

**Extensions:**
- 1a. 用户补充新照片：PhotoManager 自动压缩并追加到预览区，保存时一并上传
- 1b. 用户删除已有照片：系统调用 API 删除服务端照片并更新展示
- 3a. 用户想对新照片做 AI 识别：编辑模式下也应显示"识别账单 ✨"按钮（当有新增照片时）

---

### UC-3: AI 识别账单照片

**Primary Actor:** 用户
**Scope:** 记账模块
**Level:** Subfunction

**Preconditions:**
- 用户已添加至少一张照片

**Success Guarantee (Postconditions):**
- AI 识别结果已展示供用户确认/修改
- 用户确认后条目已创建

**Trigger:** 用户在新建/编辑表单中点击"识别账单 ✨"

**Main Success Scenario:**
1. 系统进入分析状态，显示进度指示
2. 系统将照片（已压缩的 Base64）发送给 AI 服务
3. AI 返回识别结果（商户、金额、日期、明细等）
4. 系统展示预览，用户可修改识别结果
5. 用户确认保存

**Extensions:**
- 2a. AI 服务超时或失败：系统提示错误，回到输入状态，用户仍可手动填写后保存
- 4a. 识别出多日期条目：系统提供拆分/合并选项
- 5a. 用户点击"重新拍照"：回到输入状态重新添加照片

---

### UC-4: 新建差旅条目

**Primary Actor:** 用户
**Scope:** 差旅模块
**Level:** User goal

**Preconditions:**
- 用户已登录，差旅行程已存在

**Success Guarantee (Postconditions):**
- 差旅条目已创建
- 若有照片，照片已上传并关联

**Trigger:** 用户在差旅详情中点击添加条目

**Main Success Scenario:**
1. 用户打开新建条目表单
2. 用户填写类型、日期、金额、说明等信息
3. 用户点击"保存"
4. 系统创建条目并关闭表单

**Extensions:**
- 1a. 用户添加票据照片：PhotoManager 自动压缩并显示预览
- 1b. 用户粘贴文本信息并点击"阿宝分析 ✨"：进入 AI 分析流程（→ UC-6）
- 1a+1b. 用户同时添加照片和文本：AI 分析时两者一并提交

---

### UC-5: 编辑已有差旅条目

**Primary Actor:** 用户
**Scope:** 差旅模块
**Level:** User goal

**Preconditions:**
- 用户已登录，条目已存在，用户是行程创建者

**Success Guarantee (Postconditions):**
- 条目信息已更新
- 若补充了新照片，照片已上传

**Trigger:** 用户在差旅详情中点击编辑条目

**Main Success Scenario:**
1. 系统加载条目数据到编辑表单，展示已有照片
2. 用户修改字段
3. 用户点击"保存"
4. 系统更新条目并关闭表单

**Extensions:**
- 1a. 用户补充新照片：PhotoManager 自动压缩并追加到预览区
- 1b. 用户删除已有照片：系统调用 API 删除并更新展示
- 2a. 用户粘贴补充信息并点击"阿宝分析 ✨"：AI 重新分析并填充表单

---

### UC-6: AI 分析差旅信息

**Primary Actor:** 用户
**Scope:** 差旅模块
**Level:** Subfunction

**Preconditions:**
- 用户已提供文本和/或照片

**Success Guarantee (Postconditions):**
- AI 提取的信息已填充到表单，用户可审核后保存

**Trigger:** 用户点击"阿宝分析 ✨"

**Main Success Scenario:**
1. 系统将文本和照片（已压缩的 Base64）发送给 AI 服务
2. AI 返回提取结果（类型、日期、金额、说明等）
3. 系统自动填充表单字段
4. 用户审核/修改后点击"保存"

**Extensions:**
- 2a. AI 识别出多个条目：系统弹出多条目选择面板，用户勾选后批量创建
- 1a. AI 服务失败：系统提示错误，表单保持原样，用户可手动填写

---

### UC-7: PhotoManager 处理照片（公共能力）

**Primary Actor:** 功能模块（记账/差旅/未来模块）
**Scope:** 公共 UI 组件
**Level:** Subfunction

**Stakeholders and Interests:**
- 开发者 — 复用统一的照片能力，保证跨模块一致性
- 用户 — 所有模块的照片交互体验相同

**Preconditions:**
- PhotoManager 已在 utils.js 中可用

**Success Guarantee (Postconditions):**
- 照片已选择、压缩、预览或删除，行为跨模块一致

**Trigger:** 用户在任意模块中操作照片

**Main Success Scenario:**
1. 模块初始化 PhotoManager，指定预览容器和变更回调
2. 用户通过文件选择器选择照片
3. PhotoManager 自动压缩照片
4. PhotoManager 加入待上传列表并渲染缩略图预览
5. 模块通过回调感知照片列表变化（如更新按钮状态）
6. 用户可点击缩略图上的删除按钮移除照片
7. 模块在保存时从 PhotoManager 获取文件列表用于上传

**Extensions:**
- 3a. 非图片文件或无法解码：提示并跳过，其余正常处理
- 2a. 用户多次选择：累加到已有列表
- 6a. 所有照片删完：回调通知模块，模块可据此更新 UI（如隐藏 AI 按钮）
