use rusqlite::Connection;
use std::fs;
use std::path::Path;

pub fn init_db(db_path: &str) -> Connection {
    if let Some(parent) = Path::new(db_path).parent() {
        fs::create_dir_all(parent).expect("Failed to create data directory");
    }

    let conn = Connection::open(db_path).expect("Failed to open SQLite database");

    // Enable WAL mode for concurrent reads
    conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();

    init_connection(&conn);
    conn
}

/// Initialize schema on an already-opened connection (tables + migrations).
/// Used by tests with `Connection::open_in_memory()`.
pub(crate) fn init_connection(conn: &Connection) {
    conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    create_tables(conn);
    run_migrations(conn);
}

fn run_migrations(conn: &Connection) {
    // Add avatar column to users if missing
    let has_avatar: bool = conn.prepare("SELECT avatar FROM users LIMIT 1").is_ok();
    if !has_avatar {
        conn.execute_batch("ALTER TABLE users ADD COLUMN avatar TEXT DEFAULT '';")
            .ok();
    }

    // Add changed_by to todo_changelog
    let has_changed_by: bool = conn
        .prepare("SELECT changed_by FROM todo_changelog LIMIT 1")
        .is_ok();
    if !has_changed_by {
        conn.execute_batch("ALTER TABLE todo_changelog ADD COLUMN changed_by TEXT;")
            .ok();
    }

    // Add is_collaborative to todos
    let has_todo_collab: bool = conn
        .prepare("SELECT is_collaborative FROM todos LIMIT 1")
        .is_ok();
    if !has_todo_collab {
        conn.execute_batch("ALTER TABLE todos ADD COLUMN is_collaborative INTEGER DEFAULT 0;")
            .ok();
    }

    // Add is_collaborative to routines
    let has_routine_collab: bool = conn
        .prepare("SELECT is_collaborative FROM routines LIMIT 1")
        .is_ok();
    if !has_routine_collab {
        conn.execute_batch("ALTER TABLE routines ADD COLUMN is_collaborative INTEGER DEFAULT 0;")
            .ok();
    }

    // Add category and notes to english_scenarios (Learn refactor)
    let has_category: bool = conn
        .prepare("SELECT category FROM english_scenarios LIMIT 1")
        .is_ok();
    if !has_category {
        conn.execute_batch(
            "ALTER TABLE english_scenarios ADD COLUMN category TEXT DEFAULT '英语';",
        )
        .ok();
    }
    let has_notes: bool = conn
        .prepare("SELECT notes FROM english_scenarios LIMIT 1")
        .is_ok();
    if !has_notes {
        conn.execute_batch("ALTER TABLE english_scenarios ADD COLUMN notes TEXT DEFAULT '';")
            .ok();
    }

    // Add currency to expense_entries
    let has_currency: bool = conn
        .prepare("SELECT currency FROM expense_entries LIMIT 1")
        .is_ok();
    if !has_currency {
        conn.execute_batch("ALTER TABLE expense_entries ADD COLUMN currency TEXT DEFAULT 'CAD';")
            .ok();
    }

    // Add role column to users (default 'user')
    let has_role: bool = conn.prepare("SELECT role FROM users LIMIT 1").is_ok();
    if !has_role {
        conn.execute_batch("ALTER TABLE users ADD COLUMN role TEXT DEFAULT 'user';")
            .ok();
        // Set first registered user as owner
        conn.execute(
            "UPDATE users SET role = 'owner' WHERE id = (SELECT id FROM users ORDER BY created_at ASC LIMIT 1)",
            [],
        )
        .ok();
    }
    // Migrate: ensure exactly one owner exists (promote first admin by created_at)
    let has_owner: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM users WHERE role = 'owner'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;
    if !has_owner {
        // Promote first admin to owner
        conn.execute(
            "UPDATE users SET role = 'owner' WHERE id = (SELECT id FROM users WHERE role = 'admin' ORDER BY created_at ASC LIMIT 1)",
            [],
        )
        .ok();
    }
    // Ensure boris_dev is always owner
    conn.execute(
        "UPDATE users SET role = 'owner' WHERE username = 'boris_dev'",
        [],
    )
    .ok();

    // Add status column to users (default 'active')
    let has_status: bool = conn.prepare("SELECT status FROM users LIMIT 1").is_ok();
    if !has_status {
        conn.execute_batch("ALTER TABLE users ADD COLUMN status TEXT DEFAULT 'active';")
            .ok();
    }

    // Add ai_calls_remaining for guest users (NULL = unlimited for normal users)
    let has_ai_remaining: bool = conn
        .prepare("SELECT ai_calls_remaining FROM users LIMIT 1")
        .is_ok();
    if !has_ai_remaining {
        conn.execute_batch("ALTER TABLE users ADD COLUMN ai_calls_remaining INTEGER DEFAULT NULL;")
            .ok();
    }

    // Add ai_model preference to user_settings
    let has_ai_model: bool = conn
        .prepare("SELECT ai_model FROM user_settings LIMIT 1")
        .is_ok();
    if !has_ai_model {
        conn.execute_batch("ALTER TABLE user_settings ADD COLUMN ai_model TEXT DEFAULT 'auto';")
            .ok();
    }

    // Add timezone preference to user_settings
    let has_timezone: bool = conn
        .prepare("SELECT timezone FROM user_settings LIMIT 1")
        .is_ok();
    if !has_timezone {
        conn.execute_batch(
            "ALTER TABLE user_settings ADD COLUMN timezone TEXT DEFAULT 'America/Toronto';",
        )
        .ok();
    }

    // Migrate ergou_memories categories: user_fact→fact, preference→habit, delete old categories
    let has_old_categories: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM ergou_memories WHERE category IN ('user_fact', 'preference', 'behavioral_pattern', 'conversation_highlight')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(false);
    if has_old_categories {
        conn.execute_batch(
            "UPDATE ergou_memories SET category = 'fact' WHERE category = 'user_fact';
             UPDATE ergou_memories SET category = 'habit' WHERE category = 'preference';
             DELETE FROM ergou_memories WHERE category IN ('behavioral_pattern', 'conversation_highlight');",
        )
        .ok();
    }

    // Add importance column to ergou_memories (for databases created before this feature)
    let has_importance: bool = conn
        .prepare("SELECT importance FROM ergou_memories LIMIT 1")
        .is_ok();
    if !has_importance {
        conn.execute_batch(
            "ALTER TABLE ergou_memories ADD COLUMN importance INTEGER DEFAULT 3;",
        )
        .ok();
    }
    // Add additional indexes for memories
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_ergou_memories_user_category ON ergou_memories(user_id, category);",
    )
    .ok();

    // Ensure soul_states table exists (for databases created before this feature)
    let has_soul_states: bool = conn
        .prepare("SELECT user_id FROM soul_states LIMIT 1")
        .is_ok();
    if !has_soul_states {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS soul_states (
                user_id TEXT PRIMARY KEY REFERENCES users(id),
                classical_ratio REAL NOT NULL DEFAULT 0.9,
                warmth_level REAL NOT NULL DEFAULT 0.3,
                verbosity_level REAL NOT NULL DEFAULT 0.3,
                proactivity_level REAL NOT NULL DEFAULT 0.2,
                trust_level REAL NOT NULL DEFAULT 0.1,
                relationship_stage TEXT NOT NULL DEFAULT 'stranger',
                total_interactions INTEGER NOT NULL DEFAULT 0,
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS soul_evolution_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id TEXT NOT NULL REFERENCES users(id),
                parameter TEXT NOT NULL,
                old_value REAL NOT NULL,
                new_value REAL NOT NULL,
                trigger_type TEXT NOT NULL DEFAULT 'auto',
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_soul_evo_user ON soul_evolution_log(user_id);",
        )
        .ok();
    }

    // Add image_uris column to chat_messages (T-072: multimodal support)
    let has_image_uris: bool = conn
        .prepare("SELECT image_uris FROM chat_messages LIMIT 1")
        .is_ok();
    if !has_image_uris {
        conn.execute_batch("ALTER TABLE chat_messages ADD COLUMN image_uris TEXT;")
            .ok();
    }

    // Add feedback column to chat_messages (T-048: thumbs up/down)
    let has_feedback: bool = conn
        .prepare("SELECT feedback FROM chat_messages LIMIT 1")
        .is_ok();
    if !has_feedback {
        conn.execute_batch("ALTER TABLE chat_messages ADD COLUMN feedback INTEGER;")
            .ok();
    }

    // Add conversation_id to memory_extraction_log (T-077: per-conv rate limit)
    let has_conv_id: bool = conn
        .prepare("SELECT conversation_id FROM memory_extraction_log LIMIT 1")
        .is_ok();
    if !has_conv_id {
        conn.execute_batch("ALTER TABLE memory_extraction_log ADD COLUMN conversation_id TEXT;")
            .ok();
    }
    // Index must be created after the column exists (cannot be in create_tables
    // because CREATE TABLE IF NOT EXISTS is a no-op for legacy tables)
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_mem_extract_conv ON memory_extraction_log(conversation_id, created_at);",
    )
    .ok();
}

fn create_tables(conn: &Connection) {
    conn.execute_batch(
        "
        -- Users
        CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            username TEXT UNIQUE NOT NULL,
            password_hash TEXT NOT NULL,
            display_name TEXT,
            avatar TEXT DEFAULT '',
            role TEXT DEFAULT 'user',
            status TEXT DEFAULT 'active',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        -- Todos
        CREATE TABLE IF NOT EXISTS todos (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id),
            text TEXT NOT NULL,
            content TEXT DEFAULT '',
            tab TEXT NOT NULL DEFAULT 'today',
            quadrant TEXT NOT NULL DEFAULT 'not-important-not-urgent',
            progress INTEGER DEFAULT 0,
            completed INTEGER DEFAULT 0,
            completed_at TEXT,
            deleted INTEGER DEFAULT 0,
            due_date TEXT,
            assignee TEXT DEFAULT '',
            tags TEXT DEFAULT '[]',
            sort_order REAL DEFAULT 0.0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            deleted_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_todos_user_tab ON todos(user_id, tab, deleted);

        -- Todo changelog
        CREATE TABLE IF NOT EXISTS todo_changelog (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            todo_id TEXT NOT NULL REFERENCES todos(id) ON DELETE CASCADE,
            time TEXT NOT NULL,
            field TEXT NOT NULL,
            from_val TEXT,
            to_val TEXT,
            label TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_changelog_todo ON todo_changelog(todo_id);

        -- Routines
        CREATE TABLE IF NOT EXISTS routines (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id),
            text TEXT NOT NULL,
            completed_today INTEGER DEFAULT 0,
            last_completed_date TEXT,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_routines_user ON routines(user_id);

        -- Reviews
        CREATE TABLE IF NOT EXISTS reviews (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id),
            text TEXT NOT NULL,
            frequency TEXT NOT NULL,
            frequency_config TEXT DEFAULT '{}',
            notes TEXT DEFAULT '',
            category TEXT DEFAULT '',
            last_completed TEXT,
            paused INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_reviews_user ON reviews(user_id);

        -- Sessions
        CREATE TABLE IF NOT EXISTS sessions (
            token TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id),
            created_at TEXT NOT NULL,
            expires_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user_id);

        -- Conversations (for 阿宝)
        CREATE TABLE IF NOT EXISTS conversations (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id),
            title TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            is_archived INTEGER DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_conversations_user ON conversations(user_id, updated_at DESC);

        -- Chat messages
        CREATE TABLE IF NOT EXISTS chat_messages (
            id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
            role TEXT NOT NULL,
            content_text TEXT,
            content_json TEXT,
            tool_name TEXT,
            token_count INTEGER,
            image_uris TEXT,
            feedback INTEGER,
            created_at TEXT NOT NULL,
            sequence INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_messages_conv ON chat_messages(conversation_id, sequence);

        -- Chat usage log
        CREATE TABLE IF NOT EXISTS chat_usage_log (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id),
            conversation_id TEXT NOT NULL,
            model TEXT NOT NULL,
            input_tokens INTEGER NOT NULL,
            output_tokens INTEGER NOT NULL,
            tool_calls INTEGER DEFAULT 0,
            latency_ms INTEGER NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_usage_user ON chat_usage_log(user_id, created_at DESC);

        -- English scenarios
        CREATE TABLE IF NOT EXISTS english_scenarios (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id),
            title TEXT NOT NULL,
            title_en TEXT DEFAULT '',
            description TEXT DEFAULT '',
            icon TEXT DEFAULT '📖',
            content TEXT DEFAULT '',
            status TEXT NOT NULL DEFAULT 'draft',
            archived INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_scenarios_user ON english_scenarios(user_id, archived);

        -- Friendships
        CREATE TABLE IF NOT EXISTS friendships (
            id TEXT PRIMARY KEY,
            requester_id TEXT NOT NULL REFERENCES users(id),
            addressee_id TEXT NOT NULL REFERENCES users(id),
            status TEXT NOT NULL DEFAULT 'pending',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(requester_id, addressee_id)
        );
        CREATE INDEX IF NOT EXISTS idx_friendships_users ON friendships(requester_id, addressee_id, status);

        -- Shared items
        CREATE TABLE IF NOT EXISTS shared_items (
            id TEXT PRIMARY KEY,
            sender_id TEXT NOT NULL REFERENCES users(id),
            recipient_id TEXT NOT NULL REFERENCES users(id),
            item_type TEXT NOT NULL,
            item_id TEXT NOT NULL,
            item_snapshot TEXT NOT NULL,
            message TEXT DEFAULT '',
            status TEXT NOT NULL DEFAULT 'unread',
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_shared_recipient ON shared_items(recipient_id, status);

        -- Reminders
        CREATE TABLE IF NOT EXISTS reminders (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id),
            text TEXT NOT NULL,
            remind_at TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            related_todo_id TEXT,
            repeat TEXT,
            created_at TEXT NOT NULL,
            triggered_at TEXT,
            acknowledged_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_reminders_user ON reminders(user_id, status, remind_at);
        CREATE INDEX IF NOT EXISTS idx_reminders_due ON reminders(status, remind_at);

        -- Push subscriptions
        CREATE TABLE IF NOT EXISTS push_subscriptions (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id),
            endpoint TEXT NOT NULL,
            p256dh TEXT NOT NULL,
            auth TEXT NOT NULL,
            user_agent TEXT DEFAULT '',
            created_at TEXT NOT NULL,
            UNIQUE(user_id, endpoint)
        );
        CREATE INDEX IF NOT EXISTS idx_push_user ON push_subscriptions(user_id);

        -- Notifications (in-app)
        CREATE TABLE IF NOT EXISTS notifications (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id),
            type TEXT NOT NULL DEFAULT 'reminder',
            title TEXT NOT NULL,
            body TEXT DEFAULT '',
            reminder_id TEXT,
            todo_id TEXT,
            read INTEGER DEFAULT 0,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_notifications_user ON notifications(user_id, read, created_at DESC);

        -- User settings
        CREATE TABLE IF NOT EXISTS user_settings (
            user_id TEXT PRIMARY KEY REFERENCES users(id),
            push_enabled INTEGER DEFAULT 1,
            wxpusher_uid TEXT,
            quiet_hours_start TEXT,
            quiet_hours_end TEXT,
            updated_at TEXT NOT NULL
        );

        -- Contacts
        CREATE TABLE IF NOT EXISTS contacts (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id),
            name TEXT NOT NULL,
            linked_user_id TEXT REFERENCES users(id),
            friendship_id TEXT REFERENCES friendships(id) ON DELETE SET NULL,
            note TEXT DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_contacts_user ON contacts(user_id);

        -- Todo collaborators
        CREATE TABLE IF NOT EXISTS todo_collaborators (
            id TEXT PRIMARY KEY,
            todo_id TEXT NOT NULL REFERENCES todos(id) ON DELETE CASCADE,
            user_id TEXT NOT NULL REFERENCES users(id),
            role TEXT NOT NULL DEFAULT 'collaborator',
            tab TEXT NOT NULL DEFAULT 'today',
            quadrant TEXT NOT NULL DEFAULT 'not-important-not-urgent',
            sort_order REAL DEFAULT 0.0,
            status TEXT NOT NULL DEFAULT 'active',
            created_at TEXT NOT NULL,
            UNIQUE(todo_id, user_id)
        );
        CREATE INDEX IF NOT EXISTS idx_todo_collab_user ON todo_collaborators(user_id, status);
        CREATE INDEX IF NOT EXISTS idx_todo_collab_todo ON todo_collaborators(todo_id);

        -- Routine collaborators
        CREATE TABLE IF NOT EXISTS routine_collaborators (
            id TEXT PRIMARY KEY,
            routine_id TEXT NOT NULL REFERENCES routines(id) ON DELETE CASCADE,
            user_id TEXT NOT NULL REFERENCES users(id),
            status TEXT NOT NULL DEFAULT 'active',
            created_at TEXT NOT NULL,
            UNIQUE(routine_id, user_id)
        );
        CREATE INDEX IF NOT EXISTS idx_routine_collab_user ON routine_collaborators(user_id, status);

        -- Routine completions (per-person per-day)
        CREATE TABLE IF NOT EXISTS routine_completions (
            id TEXT PRIMARY KEY,
            routine_id TEXT NOT NULL REFERENCES routines(id) ON DELETE CASCADE,
            user_id TEXT NOT NULL REFERENCES users(id),
            completed_date TEXT NOT NULL,
            created_at TEXT NOT NULL,
            UNIQUE(routine_id, user_id, completed_date)
        );
        CREATE INDEX IF NOT EXISTS idx_routine_comp ON routine_completions(routine_id, user_id);

        -- Pending confirmations
        CREATE TABLE IF NOT EXISTS pending_confirmations (
            id TEXT PRIMARY KEY,
            item_type TEXT NOT NULL,
            item_id TEXT NOT NULL,
            action TEXT NOT NULL,
            initiated_by TEXT NOT NULL REFERENCES users(id),
            initiated_at TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            resolved_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_pending_item ON pending_confirmations(item_type, item_id, status);

        -- Confirmation responses
        CREATE TABLE IF NOT EXISTS confirmation_responses (
            id TEXT PRIMARY KEY,
            confirmation_id TEXT NOT NULL REFERENCES pending_confirmations(id) ON DELETE CASCADE,
            user_id TEXT NOT NULL REFERENCES users(id),
            response TEXT NOT NULL,
            responded_at TEXT NOT NULL,
            UNIQUE(confirmation_id, user_id)
        );

        -- Discoveries (Pandora daily discovery) — kept for data preservation
        CREATE TABLE IF NOT EXISTS discoveries (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id),
            title TEXT NOT NULL,
            content TEXT NOT NULL DEFAULT '',
            emoji TEXT NOT NULL DEFAULT '🎁',
            topic_area TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'generating',
            saved INTEGER NOT NULL DEFAULT 0,
            date TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_discoveries_user_date ON discoveries(user_id, date);
        CREATE INDEX IF NOT EXISTS idx_discoveries_saved ON discoveries(user_id, saved);

        -- Expense entries (one row per spending event)
        CREATE TABLE IF NOT EXISTS expense_entries (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id),
            amount REAL NOT NULL,
            date TEXT NOT NULL,
            notes TEXT DEFAULT '',
            tags TEXT DEFAULT '[]',
            ai_processed INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_expense_user_date ON expense_entries(user_id, date DESC);

        -- Expense line items (AI-parsed receipt details)
        CREATE TABLE IF NOT EXISTS expense_items (
            id TEXT PRIMARY KEY,
            entry_id TEXT NOT NULL REFERENCES expense_entries(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            quantity REAL DEFAULT 1,
            unit_price REAL,
            amount REAL NOT NULL,
            specs TEXT DEFAULT '',
            sort_order INTEGER DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_expense_items_entry ON expense_items(entry_id);

        -- Expense photos (multiple per entry)
        CREATE TABLE IF NOT EXISTS expense_photos (
            id TEXT PRIMARY KEY,
            entry_id TEXT NOT NULL REFERENCES expense_entries(id) ON DELETE CASCADE,
            filename TEXT NOT NULL,
            storage_path TEXT NOT NULL,
            file_size INTEGER NOT NULL,
            mime_type TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_expense_photos_entry ON expense_photos(entry_id);

        -- Trips (差旅)
        CREATE TABLE IF NOT EXISTS trips (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id),
            title TEXT NOT NULL,
            destination TEXT NOT NULL DEFAULT '',
            date_from TEXT NOT NULL,
            date_to TEXT NOT NULL,
            purpose TEXT DEFAULT '',
            notes TEXT DEFAULT '',
            currency TEXT DEFAULT 'CAD',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_trips_user ON trips(user_id, date_from DESC);

        -- Trip items (按天挂载的行程条目)
        CREATE TABLE IF NOT EXISTS trip_items (
            id TEXT PRIMARY KEY,
            trip_id TEXT NOT NULL REFERENCES trips(id) ON DELETE CASCADE,
            type TEXT NOT NULL DEFAULT 'misc',
            date TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            amount REAL NOT NULL DEFAULT 0,
            currency TEXT DEFAULT 'CAD',
            reimburse_status TEXT NOT NULL DEFAULT 'pending',
            notes TEXT DEFAULT '',
            sort_order INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_trip_items_trip ON trip_items(trip_id, date, sort_order);

        -- Trip photos (票据照片)
        CREATE TABLE IF NOT EXISTS trip_photos (
            id TEXT PRIMARY KEY,
            item_id TEXT NOT NULL REFERENCES trip_items(id) ON DELETE CASCADE,
            filename TEXT NOT NULL,
            storage_path TEXT NOT NULL,
            file_size INTEGER NOT NULL,
            mime_type TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_trip_photos_item ON trip_photos(item_id);

        -- Trip collaborators (协作者)
        CREATE TABLE IF NOT EXISTS trip_collaborators (
            trip_id TEXT NOT NULL REFERENCES trips(id) ON DELETE CASCADE,
            user_id TEXT NOT NULL REFERENCES users(id),
            role TEXT NOT NULL DEFAULT 'viewer',
            created_at TEXT NOT NULL,
            PRIMARY KEY (trip_id, user_id)
        );
        CREATE INDEX IF NOT EXISTS idx_trip_collab_user ON trip_collaborators(user_id);

        -- Ergou memories (二狗记忆)
        CREATE TABLE IF NOT EXISTS ergou_memories (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id),
            category TEXT NOT NULL DEFAULT 'user_fact',
            content TEXT NOT NULL,
            importance INTEGER DEFAULT 3,
            source_conversation_id TEXT,
            created_at TEXT NOT NULL,
            last_accessed_at TEXT NOT NULL,
            access_count INTEGER DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_ergou_memories_user ON ergou_memories(user_id, last_accessed_at DESC);
        CREATE INDEX IF NOT EXISTS idx_ergou_memories_user_category ON ergou_memories(user_id, category);

        -- Security events (安全事件)
        CREATE TABLE IF NOT EXISTS security_events (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id),
            event_type TEXT NOT NULL,
            severity TEXT NOT NULL,
            description TEXT NOT NULL,
            conversation_id TEXT,
            admin_notified INTEGER DEFAULT 0,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_security_events_user ON security_events(user_id, created_at DESC);

        -- Admin audit log (管理员操作审计)
        CREATE TABLE IF NOT EXISTS admin_audit_log (
            id TEXT PRIMARY KEY,
            admin_user_id TEXT NOT NULL REFERENCES users(id),
            action_type TEXT NOT NULL,
            target_user_id TEXT,
            target_resource TEXT,
            details TEXT,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_audit_log_admin ON admin_audit_log(admin_user_id, created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_audit_log_action ON admin_audit_log(action_type, created_at DESC);

        CREATE TABLE IF NOT EXISTS client_errors (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            error_message TEXT NOT NULL,
            stack TEXT,
            app_version TEXT,
            url TEXT,
            user_agent TEXT,
            screen_size TEXT,
            network_online INTEGER DEFAULT 1,
            user_id TEXT,
            breadcrumbs TEXT,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_client_errors_created ON client_errors(created_at);

        -- Ergou people (二狗认识的人)
        CREATE TABLE IF NOT EXISTS ergou_people (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL REFERENCES users(id),
            name TEXT NOT NULL,
            relationship TEXT NOT NULL,
            nickname TEXT DEFAULT '',
            attitude TEXT DEFAULT '',
            notes TEXT DEFAULT '',
            created_by TEXT DEFAULT 'admin',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_ergou_people_user ON ergou_people(user_id);

        -- Soul state (per-user personality parameters)
        CREATE TABLE IF NOT EXISTS soul_states (
            user_id TEXT PRIMARY KEY REFERENCES users(id),
            classical_ratio REAL NOT NULL DEFAULT 0.9,
            warmth_level REAL NOT NULL DEFAULT 0.3,
            verbosity_level REAL NOT NULL DEFAULT 0.3,
            proactivity_level REAL NOT NULL DEFAULT 0.2,
            trust_level REAL NOT NULL DEFAULT 0.1,
            relationship_stage TEXT NOT NULL DEFAULT 'stranger',
            total_interactions INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        -- Soul evolution audit log
        CREATE TABLE IF NOT EXISTS soul_evolution_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT NOT NULL REFERENCES users(id),
            parameter TEXT NOT NULL,
            old_value REAL NOT NULL,
            new_value REAL NOT NULL,
            trigger_type TEXT NOT NULL DEFAULT 'auto',
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_soul_evo_user ON soul_evolution_log(user_id);

        -- Memory extraction daily log (rate limiting)
        CREATE TABLE IF NOT EXISTS memory_extraction_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT NOT NULL REFERENCES users(id),
            conversation_id TEXT,
            extracted_count INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_mem_extract_user ON memory_extraction_log(user_id);
        ",
    )
    .expect("Failed to create tables");
}

/// Daily backup: VACUUM INTO backup file
pub fn daily_backup(conn: &Connection, backup_dir: &str) {
    fs::create_dir_all(backup_dir).ok();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let backup_path = format!("{}/next-{}.db", backup_dir, today);

    // Validate backup path stays within the designated directory
    let canonical_dir = match fs::canonicalize(backup_dir) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[backup] Cannot resolve backup dir {}: {}", backup_dir, e);
            return;
        }
    };
    let full_path = canonical_dir.join(format!("next-{}.db", today));
    if !full_path.starts_with(&canonical_dir) {
        eprintln!(
            "[backup] Path traversal blocked: {:?} escapes {:?}",
            full_path, canonical_dir
        );
        return;
    }

    if !Path::new(&backup_path).exists() {
        let safe_path = full_path.to_string_lossy().replace('\'', "''");
        let sql = format!("VACUUM INTO '{}'", safe_path);
        if let Err(e) = conn.execute_batch(&sql) {
            eprintln!("Backup failed: {}", e);
        } else {
            println!("Backup created: {}", backup_path);
            cleanup_old_backups(backup_dir, 30);
        }
    }
}

fn cleanup_old_backups(backup_dir: &str, keep_days: i64) {
    let cutoff = chrono::Local::now() - chrono::Duration::days(keep_days);
    if let Ok(entries) = fs::read_dir(backup_dir) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    let modified: chrono::DateTime<chrono::Local> = modified.into();
                    if modified < cutoff {
                        fs::remove_file(entry.path()).ok();
                    }
                }
            }
        }
    }
}
