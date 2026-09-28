use serde::{Deserialize, Serialize};

use crate::model::TerminationStatusCode;

/// 支持的国际化语言枚举 (与 macOS 端保持一致，支持 24 种语言)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Language {
    #[default]
    En,
    ZhHans,
    ZhHant,
    Ja,
    Ko,
    Fr,
    De,
    Es,
    Pt,
    It,
    Ru,
    Nl,
    Pl,
    Tr,
    Ar,
    Th,
    Vi,
    Id,
    Sv,
    Da,
    Nb,
    Fi,
    Cs,
    Uk,
}

impl Language {
    pub fn from_locale_str(s: &str) -> Self {
        let s = s.to_lowercase().replace('_', "-");
        if s.starts_with("zh-hant")
            || s.starts_with("zh-tw")
            || s.starts_with("zh-hk")
            || s.starts_with("zh-mo")
        {
            Self::ZhHant
        } else if s.starts_with("zh") {
            Self::ZhHans
        } else if s.starts_with("ja") {
            Self::Ja
        } else if s.starts_with("ko") {
            Self::Ko
        } else if s.starts_with("fr") {
            Self::Fr
        } else if s.starts_with("de") {
            Self::De
        } else if s.starts_with("es") {
            Self::Es
        } else if s.starts_with("pt") {
            Self::Pt
        } else if s.starts_with("it") {
            Self::It
        } else if s.starts_with("ru") {
            Self::Ru
        } else if s.starts_with("nl") {
            Self::Nl
        } else if s.starts_with("pl") {
            Self::Pl
        } else if s.starts_with("tr") {
            Self::Tr
        } else if s.starts_with("ar") {
            Self::Ar
        } else if s.starts_with("th") {
            Self::Th
        } else if s.starts_with("vi") {
            Self::Vi
        } else if s.starts_with("id") {
            Self::Id
        } else if s.starts_with("sv") {
            Self::Sv
        } else if s.starts_with("da") {
            Self::Da
        } else if s.starts_with("nb") || s.starts_with("no") {
            Self::Nb
        } else if s.starts_with("fi") {
            Self::Fi
        } else if s.starts_with("cs") {
            Self::Cs
        } else if s.starts_with("uk") {
            Self::Uk
        } else {
            Self::En
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::En => "en",
            Self::ZhHans => "zh-Hans",
            Self::ZhHant => "zh-Hant",
            Self::Ja => "ja",
            Self::Ko => "ko",
            Self::Fr => "fr",
            Self::De => "de",
            Self::Es => "es",
            Self::Pt => "pt",
            Self::It => "it",
            Self::Ru => "ru",
            Self::Nl => "nl",
            Self::Pl => "pl",
            Self::Tr => "tr",
            Self::Ar => "ar",
            Self::Th => "th",
            Self::Vi => "vi",
            Self::Id => "id",
            Self::Sv => "sv",
            Self::Da => "da",
            Self::Nb => "nb",
            Self::Fi => "fi",
            Self::Cs => "cs",
            Self::Uk => "uk",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::En => "English",
            Self::ZhHans => "简体中文",
            Self::ZhHant => "繁體中文",
            Self::Ja => "日本語",
            Self::Ko => "한국어",
            Self::Fr => "Français",
            Self::De => "Deutsch",
            Self::Es => "Español",
            Self::Pt => "Português",
            Self::It => "Italiano",
            Self::Ru => "Русский",
            Self::Nl => "Nederlands",
            Self::Pl => "Polski",
            Self::Tr => "Türkçe",
            Self::Ar => "العربية",
            Self::Th => "ไทย",
            Self::Vi => "Tiếng Việt",
            Self::Id => "Bahasa Indonesia",
            Self::Sv => "Svenska",
            Self::Da => "Dansk",
            Self::Nb => "Norsk Bokmål",
            Self::Fi => "Suomi",
            Self::Cs => "Čeština",
            Self::Uk => "Українська",
        }
    }
}

pub fn localize_status_code(code: TerminationStatusCode, lang: Language) -> &'static str {
    match lang {
        Language::ZhHans => match code {
            TerminationStatusCode::Unknown => "未知",
            TerminationStatusCode::SuccessGraceful => "正常退出",
            TerminationStatusCode::SuccessForceTerminate => "强制终止",
            TerminationStatusCode::SkippedCallerLineage => "跳过 (调用者会话链路)",
            TerminationStatusCode::SkippedCriticalDaemon => "跳过 (系统关键进程)",
            TerminationStatusCode::FailedPermissionDenied => "失败 (权限不足)",
            TerminationStatusCode::FailedDispatch => "失败 (消息发送失败)",
            TerminationStatusCode::FailedTimeout => "失败 (超时无响应)",
            TerminationStatusCode::FailedKill => "失败 (无法强制终止)",
        },
        Language::ZhHant => match code {
            TerminationStatusCode::Unknown => "未知",
            TerminationStatusCode::SuccessGraceful => "正常退出",
            TerminationStatusCode::SuccessForceTerminate => "強制終止",
            TerminationStatusCode::SkippedCallerLineage => "跳過 (調用者會話鏈路)",
            TerminationStatusCode::SkippedCriticalDaemon => "跳過 (系統關鍵進程)",
            TerminationStatusCode::FailedPermissionDenied => "失敗 (權限不足)",
            TerminationStatusCode::FailedDispatch => "失敗 (訊息發送失敗)",
            TerminationStatusCode::FailedTimeout => "失敗 (超時無響應)",
            TerminationStatusCode::FailedKill => "失敗 (無法強制終止)",
        },
        _ => match code {
            TerminationStatusCode::Unknown => "Unknown",
            TerminationStatusCode::SuccessGraceful => "Graceful exit",
            TerminationStatusCode::SuccessForceTerminate => "Force terminated",
            TerminationStatusCode::SkippedCallerLineage => "Skipped (caller lineage)",
            TerminationStatusCode::SkippedCriticalDaemon => "Skipped (critical daemon)",
            TerminationStatusCode::FailedPermissionDenied => "Failed (permission denied)",
            TerminationStatusCode::FailedDispatch => "Failed (message dispatch failed)",
            TerminationStatusCode::FailedTimeout => "Failed (timeout)",
            TerminationStatusCode::FailedKill => "Failed (force kill failed)",
        },
    }
}
