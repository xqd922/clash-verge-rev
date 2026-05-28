pub const AUTOSTART_ARG: &str = "--autostart";

pub fn has_autostart_arg<I, S>(args: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter().any(|arg| arg.as_ref() == AUTOSTART_ARG)
}

pub fn should_silent_start<I, S>(silent_start_enabled: bool, args: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    silent_start_enabled && has_autostart_arg(args)
}

pub fn is_autostart_launch() -> bool {
    has_autostart_arg(std::env::args().skip(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disables_silent_start_when_setting_is_off_even_for_autostart() {
        assert!(!should_silent_start(false, [AUTOSTART_ARG]));
    }

    #[test]
    fn disables_silent_start_for_manual_launch_without_autostart_arg() {
        assert!(!should_silent_start(true, std::iter::empty::<&str>()));
    }

    #[test]
    fn enables_silent_start_only_for_autostart_launch() {
        assert!(should_silent_start(true, [AUTOSTART_ARG]));
    }

    #[test]
    fn ignores_deep_link_arguments_as_autostart_source() {
        assert!(!should_silent_start(
            true,
            ["clash://install-config/?url=https%3A%2F%2Fexample.com"],
        ));
    }
}
