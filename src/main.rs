use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use iced::{Element, Length, Application, Settings, Theme, executor, Command, Font, theme, Subscription, keyboard};
use iced::widget::{container, text, text_editor, column, row, horizontal_space, button, tooltip, pick_list};
use iced::highlighter::{self, Highlighter};

fn main() -> iced::Result{
    Editor::run(Settings {
        default_font: Font::MONOSPACE,
        fonts: vec![include_bytes!("../fonts/editor-icons.ttf").as_slice().into()],
        ..Settings::default()})
}

struct Editor {
    content: text_editor::Content,
    error: Option<Error>,
    path: Option<PathBuf>,
    saved: bool,
    theme: highlighter::Theme
}

#[derive(Debug, Clone)]
enum Message {
    Edit(text_editor::Action),
    Open,
    FileOpened(Result<(PathBuf, Arc<String>), Error>),
    New,
    Save,
    FileSaved(Result<PathBuf, Error>),
    NewTheme(highlighter::Theme)
}

impl Application for Editor {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new( _flags: Self::Flags) -> (Self, Command<Message>) {
        (Self {
            path: None,
            content: text_editor::Content::with(include_str!("main.rs")),
            error: None,
            saved: true,
            theme: highlighter::Theme::SolarizedDark,
        }, Command::perform(load_file(
            default_file()
            ), Message::FileOpened)
        )
    }

    fn title(&self) -> String {
        "Text Editor".to_string()
    }

    fn update(&mut self, message: Self::Message) -> Command<Message> {
        match message {

            Message::Edit(action) => {
                self.content.edit(action.clone());
                self.error = None;
                if action.is_edit() {
                    self.saved = false;
                }
                Command::none()
            }

            Message::FileOpened(Ok((path, content))) => {
                self.path = Some(path);
                self.content = text_editor::Content::with(content.as_str());
                self.saved = true;
                Command::none()
            }

            Message::FileOpened(Err(error)) => {
                self.error = Some(error);
                Command::none()
            }

            Message::Open => Command::perform(pick_file(), Message::FileOpened),

            Message::New => {
                self.path = None;
                self.content = text_editor::Content::new();
                self.saved = false;
                Command::none()
            }

            Message::Save => {
                let text = self.content.text();

                Command::perform(save_file(self.path.clone(), text), Message::FileSaved)
            }

            Message::FileSaved(Ok(path)) => {
                self.path = Some(path);
                self.saved = true;
                Command::none()
            }

            Message::FileSaved(Err(error)) => {
                self.error = Some(error);
                Command::none()
            }

            Message::NewTheme(theme) => {
                self.theme = theme;
                Command::none()
            }

        }
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        keyboard::on_key_press(|key_code, modifiers| {
            match key_code {
                keyboard::KeyCode::S if modifiers.command() => Some(Message::Save),
                _ => None,
            }
        })
    }
    
    fn view(&self) -> Element<'_, Self::Message> {
        let controls = row![
            action(new_icon(), "New File", Some(Message::New)),
            action(open_icon(), "open File", Some(Message::Open)),
            action(save_icon(), "Save File", if self.saved { None } else { Some(Message::Save) }),
            horizontal_space(Length::Fill),
            pick_list(highlighter::Theme::ALL, Some(self.theme), Message::NewTheme)
        ].spacing(10);

        let input = text_editor(&self.content)
            .on_edit(Message::Edit)
            .highlight::<Highlighter>(highlighter::Settings {
                theme: self.theme,
                extension: self.path.as_ref()
                    .and_then(|path| path.extension()?.to_str())
                    .unwrap_or("rs")
                    .to_string()
            }, |highlight, _theme | highlight.to_format());

        let status_bar = {

            let position = {
                let (line, column) = self.content.cursor_position();

                text(format!("{}:{}", line +1, column + 1))
            };

            let status = {

                let mut string = String::new();

                if let Some(Error::IOFailed(error)) = self.error.as_ref() {
                    string.push_str(error.to_string().as_str());
                } else {
                    match self.path.as_deref().and_then(Path::to_str) {
                        Some(path) => string.push_str(path),
                        None => string.push_str("New File"),
                    }
                }

                if !self.saved{
                    string.push_str(" *");
                }

                text(string).size(14)

            };

            row![status, horizontal_space(Length::Fill), position]

        };

        container(column![controls, input, status_bar].spacing(10))
            .padding(10).into()
    }

    fn theme(&self) -> Theme {
        if self.theme.is_dark() {
            Theme::Dark
        }else {
            Theme::Light
        }
    }
}

fn action<'a>(content: Element<'a, Message>, label: &str, on_press: Option<Message>) -> Element<'a, Message>{

    let is_disabled = on_press.is_none();

    tooltip(button(
        container(content)
            .width(30).center_x())
        .on_press_maybe(on_press)
                .style(
                    if is_disabled {
                        theme::Button::Secondary
                    }else {
                        theme::Button::Primary
                    }
                )
        .padding([5, 10]
        ),
            label,
            tooltip::Position::FollowCursor
    )
        .style(theme::Container::Box)
        .into()
}

fn new_icon<'a>() -> Element<'a, Message>{
    icon('\u{F0F6}')
}

fn save_icon<'a>() -> Element<'a, Message>{
    icon('\u{E800}')
}

fn open_icon<'a>() -> Element<'a, Message>{
    icon('\u{F115}')
}

fn icon<'a>(codepoint: char) -> Element<'a, Message>{
    const ICON_FONT: Font = Font::with_name("editor-icons");

    text(codepoint).font(ICON_FONT).into()
}

fn default_file() -> PathBuf {
    PathBuf::from(format!("{}/src/main.rs", env!("CARGO_MANIFEST_DIR")).as_str())
}

async fn pick_file() -> Result<(PathBuf, Arc<String>), Error> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("Choose a text file...")
        .pick_file()
        .await
        .ok_or(Error::DialogClosed)?;

    load_file(handle.path().to_path_buf()).await
}

async fn load_file(path: PathBuf) -> Result<(PathBuf, Arc<String>), Error> {
    let contents = tokio::fs::read_to_string(&path)
        .await
        .map(Arc::new)
        .map_err(|error| error.kind())
        .map_err(Error::IOFailed)?;

    Ok((path, contents))
}

async fn save_file(path: Option<PathBuf>, text: String) -> Result<PathBuf, Error> {
    let path = if let Some(path) = path { path } else {
        rfd::AsyncFileDialog::new()
            .set_title("Choose a file name...")
            .set_file_name("new.txt")
            .save_file()
            .await
            .ok_or(Error::DialogClosed)
            .map(|handle| handle.path().to_owned())?
    };

    tokio::fs::write(&path, text).await.map_err(|error| Error::IOFailed(error.kind()))?;

    Ok(path)
}

#[derive(Debug, Clone)]
enum Error {
    DialogClosed,
    IOFailed(io::ErrorKind),
}
