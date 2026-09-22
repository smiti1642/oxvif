//! Opt-in native compatibility probe. Uses only a unique synthetic account.
use std::time::{SystemTime, UNIX_EPOCH};

struct Cleanup(keyring3::Entry, keyring4::Entry);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = self.0.delete_credential();
        let _ = self.1.delete_credential();
    }
}

fn run() -> Result<(), &'static str> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "clock")?
        .as_nanos();
    let account = format!("migration-probe/{}/{nonce}", std::process::id());
    // Production identity layout, with a fresh synthetic reference only.
    let old = keyring3::Entry::new("oxvif", &account).map_err(|_| "v3 entry")?;
    let new = keyring4::Entry::new("oxvif", &account).map_err(|_| "v4 entry")?;
    if !matches!(old.get_password(), Err(keyring3::Error::NoEntry))
        || !matches!(new.get_password(), Err(keyring4::Error::NoEntry))
    {
        return Err("fresh account must be absent in both versions");
    }
    let entries = Cleanup(old, new);
    entries
        .0
        .set_password("synthetic-v3-測試")
        .map_err(|_| "v3 write")?;
    if entries
        .1
        .get_password()
        .map_err(|_| "v4 read of v3 entry")?
        != "synthetic-v3-測試"
    {
        return Err("v3 to v4 value mismatch");
    }
    entries
        .1
        .set_password("synthetic-v4-更新")
        .map_err(|_| "v4 update")?;
    if entries.0.get_password().map_err(|_| "v3 rollback read")? != "synthetic-v4-更新" {
        return Err("v4 to v3 value mismatch");
    }
    entries.1.delete_credential().map_err(|_| "v4 delete")?;
    if !matches!(entries.0.get_password(), Err(keyring3::Error::NoEntry)) {
        return Err("v4 deletion not visible to v3");
    }
    entries
        .1
        .set_password("synthetic-v4-new")
        .map_err(|_| "v4 create")?;
    if entries
        .0
        .get_password()
        .map_err(|_| "v3 read of v4 entry")?
        != "synthetic-v4-new"
    {
        return Err("v4 creation not visible to v3");
    }
    entries.0.delete_credential().map_err(|_| "v3 delete")?;
    if !matches!(entries.1.get_password(), Err(keyring4::Error::NoEntry))
        || !matches!(entries.0.get_password(), Err(keyring3::Error::NoEntry))
    {
        return Err("cleanup verification");
    }
    Ok(())
}

fn main() {
    match run() {
        Ok(()) => println!(
            "PASS: keyring 3.6.3 <-> 4.2.0 v1 facade; native create/read/update/rollback/delete; synthetic account removed"
        ),
        Err(stage) => {
            eprintln!(
                "FAIL: native migration stage: {stage}; raw backend errors and values suppressed"
            );
            std::process::exit(1);
        }
    }
}
