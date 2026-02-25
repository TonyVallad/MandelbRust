/// Top-level screen the application is currently displaying.
///
/// Used to dispatch `update()` to the right screen-drawing logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AppScreen {
    /// Main menu shown at startup.
    MainMenu,
    /// The main fractal rendering / exploration view.
    FractalExplorer,
    /// Full-window bookmark browser (accessed from main menu).
    BookmarkBrowser,
    /// Full-window Julia C Explorer (accessed from main menu).
    JuliaCExplorer,
}

impl Default for AppScreen {
    fn default() -> Self {
        Self::MainMenu
    }
}
