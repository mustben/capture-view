//! Device-independent selection policy; unreadable third-party devices are skipped.
#[derive(Default)]
pub struct Selection<T> {
    matches: Vec<(T, String)>,
    unreadable: usize,
}
impl<T> Selection<T> {
    pub fn new() -> Self {
        Self {
            matches: Vec::new(),
            unreadable: 0,
        }
    }
    pub fn observe(&mut self, device: T, name: Result<String, ()>) {
        match name {
            Ok(name) if name.to_ascii_lowercase().contains("avermedia") => {
                self.matches.push((device, name))
            }
            Ok(_) => {}
            Err(()) => self.unreadable += 1,
        }
    }
    pub fn finish(mut self) -> Result<(T, String), &'static str> {
        match self.matches.len() {
            1 => Ok(self.matches.pop().unwrap()),
            0 if self.unreadable > 0 => Err(
                "Recherche incomplète : des périphériques sont illisibles ; aucun AVerMedia lisible détecté. Consultez le journal.",
            ),
            0 => Err("Aucun boîtier AVerMedia détecté. Vérifiez le câble USB et le pilote."),
            _ => Err("Plusieurs boîtiers AVerMedia détectés. Ne branchez qu'un seul boîtier."),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn skips_unreadable_and_unrelated_devices() {
        let mut s = Selection::new();
        s.observe(1, Err(()));
        s.observe(2, Ok("Webcam".into()));
        s.observe(3, Ok("AVerMedia GC550".into()));
        assert_eq!(s.finish().unwrap().0, 3);
    }
    #[test]
    fn absence_incomplete_and_ambiguous_are_distinct() {
        let absent = Selection::<()>::new().finish().unwrap_err();
        let mut incomplete = Selection::new();
        incomplete.observe((), Err(()));
        assert_ne!(absent, incomplete.finish().unwrap_err());
        let mut multiple = Selection::new();
        multiple.observe((), Ok("AVerMedia A".into()));
        multiple.observe((), Ok("AVerMedia B".into()));
        assert!(multiple.finish().unwrap_err().contains("Plusieurs"));
    }
}
