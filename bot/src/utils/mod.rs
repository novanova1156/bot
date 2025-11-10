// bot/src/utils/mod.rs
pub mod math;
pub mod rpc;

use anyhow::{Context, Result};
use solana_sdk::signature::Keypair;
use std::fs;
use std::str::FromStr;
use solana_sdk::pubkey::Pubkey;

/// Загрузка keypair из файла
pub fn load_keypair_from_file(path: &str) -> Result<Keypair> {
    let expanded_path = shellexpand::tilde(path).into_owned();

    let keypair_bytes = fs::read(&expanded_path)
        .context(format!("Не удалось прочитать файл кошелька: {}", expanded_path))?;

    let keypair_data: Vec<u8> = serde_json::from_slice(&keypair_bytes)
        .context("Неверный формат файла кошелька (ожидается JSON массив байт)")?;

    // ИСПРАВЛЕНИЕ: используем try_from вместо устаревшего from_bytes
    let keypair = Keypair::try_from(&keypair_data[..])
        .map_err(|e| anyhow::anyhow!("Не удалось создать keypair из байтов: {}. Убедитесь, что файл содержит 64 байта.", e))?;

    Ok(keypair)
}

/// Форматирование lamports в SOL с заданной точностью
pub fn lamports_to_sol(lamports: u64, decimals: usize) -> String {
    let sol = lamports as f64 / 1_000_000_000.0;
    format!("{:.decimals$}", sol, decimals = decimals)
}

/// Конвертация SOL в lamports
pub fn sol_to_lamports(sol: f64) -> u64 {
    (sol * 1_000_000_000.0) as u64
}

/// Проверка валидности Pubkey
pub fn is_valid_pubkey(address: &str) -> bool {
    Pubkey::from_str(address).is_ok()
}