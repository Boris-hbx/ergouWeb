# TEST-todo-work-upgrade

> Task: T-273
> Scope: Todo one-way upgrade into the work task table

## Backend

- `todos` includes `upgraded_to_work`, `work_task_id`, and `upgraded_at` with backward-compatible defaults.
- `work_tasks` includes `source_type` and `source_todo_id` with `source_type='manual'` for normal tasks.
- `GET /api/todos` and `GET /api/todos/:id` expose `upgradedToWork`, `workTaskId`, and `upgradedAt`.
- `GET /api/work/tasks` exposes `sourceType` and `sourceTodoId`.
- `POST /api/todos/:id/upgrade-to-work` creates one work task for an unupgraded Todo and returns both the updated Todo and WorkTask.
- Upgrade maps `text -> title`, `content -> desc`, `due_date -> due`, `progress -> progress`, `completed -> status`, `tags -> tags`, and `quadrant -> priority`.
- Calling the upgrade endpoint again for the same Todo returns the existing work task and does not create a duplicate.
- If the linked work task is missing or soft-deleted, upgrade creates a new work task and overwrites the Todo link.
- Editing or deleting a Todo-sourced work task does not update or delete the source Todo.

## Frontend

- Todo list/detail shows an "升级到工作任务" action for unupgraded Todos.
- Upgraded Todos show an "已升级" badge and can jump to the linked work task.
- Re-clicking the upgrade action on an upgraded Todo jumps to the existing work task instead of creating another one.
- Todo page provides a "隐藏已升级项" toggle that filters upgraded Todos locally.
- Work task table and detail drawer show "来自 Todo" for Todo-sourced tasks.
- Work task table can filter source by "手动" and "Todo".
- Todo upgrade wording only refers to the work task table and does not imply upgrade to life, learn, or other modules.

## Compatibility

- Existing Todo create, edit, complete, delete, restore, quadrant, and tag filters continue to work.
- Existing work task views, detail drawer, column editing, and LLM tool reuse continue to work.
- No new unified task page is introduced.
- No WorkTask-to-Todo sync path is introduced.
