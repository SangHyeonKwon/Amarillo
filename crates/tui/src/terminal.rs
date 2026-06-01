//! 터미널 setup/restore + panic hook.
//!
//! TUI는 alt-screen + raw mode로 터미널을 점유한다. 정상 종료뿐 아니라 **패닉
//! 시에도** 반드시 원복해야 사용자의 셸이 깨지지 않는다 — [`install_panic_hook`].

use std::io::{self, Stdout};

use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::error::TuiError;

/// 본 앱이 쓰는 구체 터미널 타입.
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// raw mode 진입 + alt-screen 전환 후 터미널 핸들을 만든다.
pub fn setup() -> Result<Tui, TuiError> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// alt-screen 이탈 + raw mode 해제 + 커서 복원. best-effort (정상 경로).
pub fn restore(terminal: &mut Tui) -> Result<(), TuiError> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

/// 패닉 훅 설치 — 백트레이스 출력 전에 터미널을 원복한다.
///
/// 기존 훅을 보존했다가 원복 후 호출하므로, 패닉 메시지는 정상 터미널에 찍힌다.
pub fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // 원복 실패는 무시 — 어차피 패닉 경로이므로 최선 노력만 한다.
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original(info);
    }));
}
