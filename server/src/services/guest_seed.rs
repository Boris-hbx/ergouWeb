use crate::state::AppState;
use std::fs;
use std::path::Path;

/// Seed demo data for a new guest user.
pub fn seed_guest_demo_data(state: &AppState, user_id: &str) {
    let db = state.db.lock();
    let now = chrono::Utc::now().to_rfc3339();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    // ── Todos (11 items across today/week/month) ──
    seed_todos(&db, user_id, &now, &today);

    // ── Routines (8 items) ──
    seed_routines(&db, user_id, &now, &today);

    // ── Reviews (2 items) ──
    seed_reviews(&db, user_id, &now);

    // ── Expenses with photos (3 entries) ──
    seed_expenses(&db, user_id, &now, &today);

    // ── Trip with proper items (1 trip, 8 items) ──
    seed_trip(&db, user_id, &now);

    // ── English scenario (airport lost & found) ──
    seed_english(&db, user_id, &now);
}

fn seed_todos(db: &rusqlite::Connection, user_id: &str, now: &str, today: &str) {
    let week_date = (chrono::Local::now() + chrono::Duration::days(2))
        .format("%Y-%m-%d")
        .to_string();
    let month_date = (chrono::Local::now() + chrono::Duration::days(14))
        .format("%Y-%m-%d")
        .to_string();

    // (text, tab, quadrant, progress, completed, sort_order, due_date)
    #[allow(clippy::type_complexity)]
    let todos: Vec<(&str, &str, &str, i32, i32, f64, Option<&str>)> = vec![
        // ── Today ──
        (
            "完成项目报告",
            "today",
            "important-urgent",
            40,
            0,
            1.0,
            Some(today),
        ),
        (
            "回复客户邮件",
            "today",
            "important-urgent",
            0,
            0,
            2.0,
            Some(today),
        ),
        (
            "准备周报",
            "today",
            "important-not-urgent",
            100,
            1,
            1.0,
            Some(today),
        ),
        (
            "修复登录页面 bug",
            "today",
            "not-important-urgent",
            60,
            0,
            1.0,
            Some(today),
        ),
        // ── This Week ──
        (
            "学习 Rust 异步编程",
            "week",
            "important-not-urgent",
            20,
            0,
            1.0,
            Some(&week_date),
        ),
        (
            "Code Review 新功能分支",
            "week",
            "important-urgent",
            0,
            0,
            1.0,
            Some(&week_date),
        ),
        (
            "整理技术文档",
            "week",
            "important-not-urgent",
            0,
            0,
            2.0,
            Some(&week_date),
        ),
        (
            "团队周会准备 PPT",
            "week",
            "not-important-urgent",
            0,
            0,
            1.0,
            Some(&week_date),
        ),
        // ── Next 30 Days ──
        (
            "更新个人简历",
            "month",
            "important-not-urgent",
            10,
            0,
            1.0,
            Some(&month_date),
        ),
        (
            "预约体检",
            "month",
            "not-important-not-urgent",
            0,
            0,
            1.0,
            Some(&month_date),
        ),
        (
            "完成在线课程第三章",
            "month",
            "important-not-urgent",
            0,
            0,
            2.0,
            Some(&month_date),
        ),
    ];

    for (text, tab, quadrant, progress, completed, sort_order, due) in &todos {
        let id = uuid::Uuid::new_v4().to_string();
        let completed_at: Option<&str> = if *completed == 1 { Some(now) } else { None };
        db.execute(
            "INSERT INTO todos (id, user_id, text, tab, quadrant, progress, completed, completed_at, due_date, sort_order, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![id, user_id, text, tab, quadrant, progress, completed, completed_at, due, sort_order, now, now],
        )
        .ok();
    }
}

fn seed_routines(db: &rusqlite::Connection, user_id: &str, now: &str, today: &str) {
    let routines: Vec<(&str, bool)> = vec![
        ("每日：上班打卡", true),
        ("每周：给爸妈打电话", false),
        ("每周：锻炼 3 次", false),
        ("每月：还 xxx 信用卡", false),
        ("每月：交 xxx 账单", false),
        ("每年：老婆生日", false),
        ("每年：爸妈生日", false),
        ("每年：纪念日", false),
    ];

    for (text, completed_today) in routines {
        let id = uuid::Uuid::new_v4().to_string();
        let completed_val = if completed_today { 1 } else { 0 };
        let last_date: Option<&str> = if completed_today { Some(today) } else { None };
        db.execute(
            "INSERT INTO routines (id, user_id, text, completed_today, last_completed_date, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![id, user_id, text, completed_val, last_date, now],
        )
        .ok();
    }
}

fn seed_reviews(db: &rusqlite::Connection, user_id: &str, now: &str) {
    let reviews: Vec<(&str, &str)> = vec![
        ("每周回顾目标进度", "weekly"),
        ("整理笔记和收藏", "monthly"),
    ];

    for (text, frequency) in reviews {
        let id = uuid::Uuid::new_v4().to_string();
        db.execute(
            "INSERT INTO reviews (id, user_id, text, frequency, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![id, user_id, text, frequency, now, now],
        )
        .ok();
    }
}

fn seed_expenses(db: &rusqlite::Connection, user_id: &str, now: &str, today: &str) {
    let db_dir = std::env::var("DATABASE_PATH")
        .unwrap_or_else(|_| "data/next.db".to_string())
        .replace("/next.db", "")
        .replace("\\next.db", "");
    let upload_dir = format!("{}/uploads/{}", db_dir, user_id);
    fs::create_dir_all(&upload_dir).ok();

    // Demo photo source directory
    let demo_dir = if Path::new("/app/data/demo-photos").exists() {
        "/app/data/demo-photos".to_string() // Docker
    } else {
        // Local dev: use testdata directly
        let base = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
        let candidate = format!("{}/../data/demo-photos", base);
        if Path::new(&candidate).exists() {
            candidate
        } else {
            "data/demo-photos".to_string()
        }
    };

    // Group a: T&T 大统华 (4 photos)
    let entry_a_id = uuid::Uuid::new_v4().to_string();
    db.execute(
        "INSERT INTO expense_entries (id, user_id, amount, date, notes, tags, ai_processed, created_at, updated_at) \
         VALUES (?1, ?2, 0, ?3, 'T&T 大统华超市 - 4张收据照片，试试二狗分析', '[\"超市\"]', 0, ?4, ?5)",
        rusqlite::params![entry_a_id, user_id, today, now, now],
    )
    .ok();
    let photos_a = [
        "expense-a-1.jpg",
        "expense-a-2.jpg",
        "expense-a-3.jpg",
        "expense-a-4.jpg",
    ];
    copy_demo_photos(
        &demo_dir,
        &upload_dir,
        &photos_a,
        db,
        &entry_a_id,
        now,
        "expense_photos",
        "entry_id",
    );

    // Group b: Costco gas (1 photo)
    let entry_b_id = uuid::Uuid::new_v4().to_string();
    db.execute(
        "INSERT INTO expense_entries (id, user_id, amount, date, notes, tags, ai_processed, created_at, updated_at) \
         VALUES (?1, ?2, 0, ?3, 'Costco 加油 - 试试二狗分析', '[\"加油\"]', 0, ?4, ?5)",
        rusqlite::params![entry_b_id, user_id, today, now, now],
    )
    .ok();
    let photos_b = ["expense-b-1.jpg"];
    copy_demo_photos(
        &demo_dir,
        &upload_dir,
        &photos_b,
        db,
        &entry_b_id,
        now,
        "expense_photos",
        "entry_id",
    );

    // Group c: Mixed receipts (4 photos)
    let entry_c_id = uuid::Uuid::new_v4().to_string();
    db.execute(
        "INSERT INTO expense_entries (id, user_id, amount, date, notes, tags, ai_processed, created_at, updated_at) \
         VALUES (?1, ?2, 0, ?3, '多店混合票据 - 4张不同收据，试试二狗分析', '[]', 0, ?4, ?5)",
        rusqlite::params![entry_c_id, user_id, today, now, now],
    )
    .ok();
    let photos_c = [
        "expense-c-1.jpg",
        "expense-c-2.jpg",
        "expense-c-3.jpg",
        "expense-c-4.jpg",
    ];
    copy_demo_photos(
        &demo_dir,
        &upload_dir,
        &photos_c,
        db,
        &entry_c_id,
        now,
        "expense_photos",
        "entry_id",
    );
}

#[allow(clippy::too_many_arguments)]
fn copy_demo_photos(
    demo_dir: &str,
    upload_dir: &str,
    filenames: &[&str],
    db: &rusqlite::Connection,
    parent_id: &str,
    now: &str,
    table: &str,
    fk_col: &str,
) {
    for src_name in filenames {
        let src_path = format!("{}/{}", demo_dir, src_name);
        if !Path::new(&src_path).exists() {
            eprintln!("Demo photo not found: {}", src_path);
            continue;
        }

        let photo_id = uuid::Uuid::new_v4().to_string();
        // Use same naming convention as normal upload: {photo_id}.jpg
        let ext = src_name.rsplit('.').next().unwrap_or("jpg");
        let storage_name = format!("{}.{}", photo_id, ext);
        let dst_path = format!("{}/{}", upload_dir, storage_name);

        if fs::copy(&src_path, &dst_path).is_err() {
            eprintln!("Failed to copy demo photo: {} -> {}", src_path, dst_path);
            continue;
        }

        let file_size = fs::metadata(&dst_path).map(|m| m.len() as i64).unwrap_or(0);
        let sql = format!(
            "INSERT INTO {} (id, {}, filename, storage_path, file_size, mime_type, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, 'image/jpeg', ?6)",
            table, fk_col
        );
        db.execute(
            &sql,
            rusqlite::params![photo_id, parent_id, storage_name, dst_path, file_size, now],
        )
        .ok();
    }
}

fn seed_trip(db: &rusqlite::Connection, user_id: &str, now: &str) {
    let trip_id = uuid::Uuid::new_v4().to_string();
    let day0 = chrono::Local::now();
    let date_from = day0.format("%Y-%m-%d").to_string();
    let day1 = (day0 + chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();
    let day2 = (day0 + chrono::Duration::days(2))
        .format("%Y-%m-%d")
        .to_string();
    let date_to = day2.clone();

    db.execute(
        "INSERT INTO trips (id, user_id, title, destination, date_from, date_to, purpose, notes, currency, created_at, updated_at) \
         VALUES (?1, ?2, '出差 - 上海', '上海', ?3, ?4, '客户拜访', '', 'CNY', ?5, ?6)",
        rusqlite::params![trip_id, user_id, date_from, date_to, now, now],
    )
    .ok();

    // Trip items: (type, date, description, amount, notes, sort_order)
    let items: Vec<(&str, &str, &str, f64, &str, i32)> = vec![
        // Day 1
        (
            "flight",
            &date_from,
            "MU5101 北京首都T2 → 上海虹桥T2 08:00-10:20",
            1280.0,
            "经济舱",
            0,
        ),
        ("taxi", &date_from, "虹桥机场 → 万豪酒店", 45.0, "", 1),
        (
            "hotel",
            &date_from,
            "上海万豪虹桥酒店（3晚）",
            2040.0,
            "¥680/晚 × 3",
            2,
        ),
        ("meal", &date_from, "午餐", 86.0, "", 3),
        // Day 2
        ("taxi", &day1, "酒店 → 客户公司", 32.0, "", 0),
        ("meal", &day1, "午餐 客户陪同", 120.0, "", 1),
        ("meal", &day1, "晚餐 团队聚餐", 256.0, "", 2),
        // Day 3
        ("taxi", &day2, "酒店 → 虹桥机场", 45.0, "", 0),
        (
            "flight",
            &day2,
            "MU5108 上海虹桥T2 → 北京首都T2 19:00-21:20",
            1350.0,
            "经济舱",
            1,
        ),
    ];

    for (item_type, date, desc, amount, notes, sort_order) in &items {
        let item_id = uuid::Uuid::new_v4().to_string();
        db.execute(
            "INSERT INTO trip_items (id, trip_id, type, date, description, amount, currency, notes, sort_order, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'CNY', ?7, ?8, ?9, ?10)",
            rusqlite::params![item_id, trip_id, item_type, date, desc, amount, notes, sort_order, now, now],
        )
        .ok();
    }
}

fn seed_english(db: &rusqlite::Connection, user_id: &str, now: &str) {
    let id = uuid::Uuid::new_v4().to_string();

    let content = "\
## 🎬 场景介绍

你刚下飞机到达多伦多皮尔逊国际机场，发现托运的行李没有出现在传送带上。你需要前往机场失物招领处（Baggage Services）寻求帮助。

## 💬 核心对话

**You:** Excuse me, I can't find my checked baggage. It didn't come out on the carousel.

*不好意思，我找不到我的托运行李。它没有出现在传送带上。*

**Staff:** I'm sorry to hear that. Can I see your baggage claim tag, please?

*很抱歉听到这个消息。请出示您的行李提取标签好吗？*

**You:** Sure, here it is. I was on Flight AC026 from Shanghai.

*当然，给您。我乘坐的是从上海来的 AC026 航班。*

**Staff:** Let me look that up in our system. Could you describe your luggage for me?

*让我在系统里查一下。您能描述一下您的行李吗？*

**You:** It's a large black Samsonite suitcase with a red ribbon tied to the handle.

*是一个黑色的新秀丽大号行李箱，把手上系了一根红丝带。*

**Staff:** I see. It appears your bag was left behind in Shanghai and will arrive on the next flight tomorrow morning.

*我查到了。您的行李被留在了上海，将搭乘明天早上的下一班航班到达。*

**You:** Oh no. Is there anything you can do? I need some of my things tonight.

*天哪。你们能做些什么吗？我今晚需要用一些东西。*

**Staff:** We can provide you with a toiletry kit and we'll deliver your bag to your hotel once it arrives. Could you fill out this delayed baggage form?

*我们可以提供一个洗漱包，行李到达后我们会送到您的酒店。您能填一下这张行李延误表格吗？*

**You:** Yes, of course. Will I be compensated for the delay?

*好的，当然。延误会有赔偿吗？*

**Staff:** You can submit receipts for essential items up to $150 CAD, and we'll reimburse you.

*您可以提交必需品的收据，最高 150 加元，我们会给您报销。*

## 📝 常用词汇

- **checked baggage** /tʃɛkt ˈbæɡɪdʒ/ — 托运行李
- **carousel** /ˌkærəˈsɛl/ — （行李）传送带
- **baggage claim tag** /ˈbæɡɪdʒ kleɪm tæɡ/ — 行李提取标签
- **delayed baggage form** /dɪˈleɪd ˈbæɡɪdʒ fɔːrm/ — 行李延误表格
- **toiletry kit** /ˈtɔɪlətri kɪt/ — 洗漱用品包
- **reimburse** /ˌriːɪmˈbɜːrs/ — 报销
- **essential items** /ɪˈsɛnʃəl ˈaɪtəmz/ — 必需品

## 💡 实用表达

- **I can't find my...** 是描述丢失物品的常用句式，语气礼貌不急躁
- **Can I see your...?** 工作人员请求出示证件/凭证的标准用语
- **It appears that...** 委婉地传达不太好的消息，比直说更礼貌
- **Is there anything you can do?** 请求帮助的万能句，适用于各种服务场景
- **Fill out this form** 填写表格，机场/酒店/医院常用场景";

    db.execute(
        "INSERT INTO english_scenarios (id, user_id, title, title_en, description, icon, content, status, category, notes, created_at, updated_at) \
         VALUES (?1, ?2, '机场失物招领', 'Airport Lost Baggage', '在机场用英语处理行李丢失、描述物品、填写表格', '✈️', ?3, 'ready', '英语', '二狗预生成的示例场景', ?4, ?5)",
        rusqlite::params![id, user_id, content, now, now],
    )
    .ok();
}

/// Clean up expired guest users (no valid sessions left).
/// Should be called periodically (e.g. every hour).
pub fn cleanup_expired_guests(state: &AppState) {
    let db = state.db.lock();

    // Find guest users with no valid sessions
    let mut stmt = db
        .prepare(
            "SELECT id FROM users WHERE status = 'guest' \
             AND NOT EXISTS (SELECT 1 FROM sessions s WHERE s.user_id = users.id AND s.expires_at > datetime('now'))",
        )
        .unwrap();
    let guest_ids: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .unwrap()
        .flatten()
        .collect();
    drop(stmt);

    if guest_ids.is_empty() {
        return;
    }

    let db_dir = std::env::var("DATABASE_PATH")
        .unwrap_or_else(|_| "data/next.db".to_string())
        .replace("/next.db", "")
        .replace("\\next.db", "");

    for guest_id in &guest_ids {
        // Delete in dependency order within a transaction
        if db.execute_batch("BEGIN TRANSACTION").is_err() {
            continue;
        }

        // Trip photos → trip items → trip collaborators → trips
        db.execute(
            "DELETE FROM trip_photos WHERE item_id IN (SELECT id FROM trip_items WHERE trip_id IN (SELECT id FROM trips WHERE user_id = ?1))",
            [guest_id],
        ).ok();
        db.execute(
            "DELETE FROM trip_items WHERE trip_id IN (SELECT id FROM trips WHERE user_id = ?1)",
            [guest_id],
        )
        .ok();
        db.execute(
            "DELETE FROM trip_collaborators WHERE trip_id IN (SELECT id FROM trips WHERE user_id = ?1)",
            [guest_id],
        ).ok();
        db.execute("DELETE FROM trips WHERE user_id = ?1", [guest_id])
            .ok();

        // Expense photos → expense items → expense entries
        db.execute(
            "DELETE FROM expense_photos WHERE entry_id IN (SELECT id FROM expense_entries WHERE user_id = ?1)",
            [guest_id],
        ).ok();
        db.execute(
            "DELETE FROM expense_items WHERE entry_id IN (SELECT id FROM expense_entries WHERE user_id = ?1)",
            [guest_id],
        ).ok();
        db.execute("DELETE FROM expense_entries WHERE user_id = ?1", [guest_id])
            .ok();

        // Chat messages → conversations
        db.execute(
            "DELETE FROM chat_messages WHERE conversation_id IN (SELECT id FROM conversations WHERE user_id = ?1)",
            [guest_id],
        ).ok();
        db.execute("DELETE FROM conversations WHERE user_id = ?1", [guest_id])
            .ok();
        db.execute("DELETE FROM chat_usage_log WHERE user_id = ?1", [guest_id])
            .ok();

        // Todo changelog → todos
        db.execute(
            "DELETE FROM todo_changelog WHERE todo_id IN (SELECT id FROM todos WHERE user_id = ?1)",
            [guest_id],
        )
        .ok();
        db.execute("DELETE FROM todos WHERE user_id = ?1", [guest_id])
            .ok();

        // Others
        db.execute("DELETE FROM routines WHERE user_id = ?1", [guest_id])
            .ok();
        db.execute(
            "DELETE FROM routine_completions WHERE user_id = ?1",
            [guest_id],
        )
        .ok();
        db.execute("DELETE FROM reviews WHERE user_id = ?1", [guest_id])
            .ok();
        db.execute(
            "DELETE FROM english_scenarios WHERE user_id = ?1",
            [guest_id],
        )
        .ok();
        db.execute("DELETE FROM notifications WHERE user_id = ?1", [guest_id])
            .ok();
        db.execute("DELETE FROM reminders WHERE user_id = ?1", [guest_id])
            .ok();
        db.execute(
            "DELETE FROM push_subscriptions WHERE user_id = ?1",
            [guest_id],
        )
        .ok();
        db.execute("DELETE FROM user_settings WHERE user_id = ?1", [guest_id])
            .ok();
        db.execute("DELETE FROM sessions WHERE user_id = ?1", [guest_id])
            .ok();
        db.execute("DELETE FROM users WHERE id = ?1", [guest_id])
            .ok();

        db.execute_batch("COMMIT").ok();

        // Remove uploaded files
        let upload_path = format!("{}/uploads/{}", db_dir, guest_id);
        if Path::new(&upload_path).exists() {
            fs::remove_dir_all(&upload_path).ok();
        }

        println!("Cleaned up expired guest: {}", guest_id);
    }
}
