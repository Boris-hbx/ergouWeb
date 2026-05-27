## Use Cases

### Use Case: 浏览设置页找到目标设置项

**Primary Actor:** 用户
**Scope:** 设置页 UI
**Level:** User goal

**Preconditions:**
- 用户已登录，进入设置页

**Success Guarantee (Postconditions):**
- 用户在一屏半以内看到所有设置分组，快速定位并操作目标设置项

**Trigger:** 用户点击底部导航"设置" tab

**Main Success Scenario:**
1. 用户进入设置页，看到紧凑排列的设置分组
2. 账户信息区同时展示用户名、昵称和修改密码入口，无需额外滚动
3. 用户一眼识别各分组边界，间距均匀不松散
4. 用户滚动到页底，退出登录按钮嵌在应用信息区内，页面到此结束

**Extensions:**
- 1a. 页面高度超过 1.5 屏：违反紧凑化目标，需进一步收紧

---

### Use Case: 更换头像

**Primary Actor:** 用户
**Scope:** 设置页 — 头像选择器
**Level:** User goal

**Preconditions:**
- 用户在设置页，头像区可见

**Success Guarantee (Postconditions):**
- 头像已更新，预览圆显示新头像

**Trigger:** 用户想更换头像

**Main Success Scenario:**
1. 用户看到头像区：左侧当前头像预览，右侧显示 8 个预设头像
2. 用户点击某个预设头像，预览圆立即更新
3. 系统保存头像选择

**Extensions:**
- 1a. 用户想看更多预设：点击"展开"，网格展开显示全部 25 个预设
- 1b. 展开状态下点击"收起"，网格回到 8 个预设
- 2a. 用户选择上传自定义头像：点击上传按钮，选择图片，预览更新

---

### Use Case: 管理好友与联系人

**Primary Actor:** 用户
**Scope:** 设置页 — 好友/联系人区
**Level:** User goal

**Preconditions:**
- 用户在设置页

**Success Guarantee (Postconditions):**
- 好友列表和联系人列表在同一卡片内清晰可辨

**Trigger:** 用户想查看好友或管理联系人

**Main Success Scenario:**
1. 用户看到"好友"卡片，好友列表和联系人列表在同一区域内，用分隔线区分
2. 用户在好友列表中查看/操作好友
3. 用户向下看到联系人列表，无需跨卡片寻找

**Extensions:**
- 1a. 好友列表为空：显示空状态提示，联系人列表仍可见
- 1b. 联系人列表为空：仅显示好友列表，联系人区域折叠

---

### Use Case: 修改密码

**Primary Actor:** 用户
**Scope:** 设置页 — 账户信息区
**Level:** Subfunction

**Preconditions:**
- 用户在设置页，账户信息区可见

**Success Guarantee (Postconditions):**
- 密码修改流程正常触发

**Trigger:** 用户想修改密码

**Main Success Scenario:**
1. 用户在"账户信息"区看到用户名、昵称，以及"修改密码"按钮
2. 用户点击修改密码按钮
3. 系统弹出密码修改对话框

**Extensions:**
- 1a. 修改密码按钮不存在于独立卡片中——用户不必滚动到下一个 section 才找到它

---

### Use Case: 退出登录

**Primary Actor:** 用户
**Scope:** 设置页 — 应用信息区
**Level:** Subfunction

**Preconditions:**
- 用户在设置页底部

**Success Guarantee (Postconditions):**
- 退出操作正常触发

**Trigger:** 用户想退出登录

**Main Success Scenario:**
1. 用户滚动到页底，在应用信息区（App 名称、版本号）下方看到"退出登录"按钮
2. 用户点击退出按钮
3. 系统执行退出流程

**Extensions:**
- 1a. 退出按钮样式仍为红色危险按钮，视觉突出不会因合并而被忽略
