use rust_decimal::Decimal;
use serde::Serialize;

use crate::util::dec_to_f64;

#[derive(Clone)]
pub struct MemberBalance {
    pub user_id: i64,
    pub nickname: String,
    pub avatar: Option<String>,
    pub paid: Decimal,
    pub owed: Decimal,
}

#[derive(Serialize)]
pub struct SettleUserVo {
    pub user_id: i64,
    pub nickname: String,
    pub avatar: Option<String>,
    pub paid: f64,
    pub owed: f64,
    pub net: f64,
}

#[derive(Serialize)]
pub struct TransferVo {
    pub from_user_id: i64,
    pub from_nickname: String,
    pub to_user_id: i64,
    pub to_nickname: String,
    pub amount: f64,
}

pub fn calc_transfers(members: &[MemberBalance]) -> (Vec<SettleUserVo>, Vec<TransferVo>) {
    let users: Vec<SettleUserVo> = members
        .iter()
        .map(|m| {
            let net = m.paid - m.owed;
            SettleUserVo {
                user_id: m.user_id,
                nickname: m.nickname.clone(),
                avatar: m.avatar.clone(),
                paid: dec_to_f64(m.paid),
                owed: dec_to_f64(m.owed),
                net: dec_to_f64(net),
            }
        })
        .collect();

    let mut debtors: Vec<(i64, Decimal)> = members
        .iter()
        .filter(|m| m.paid < m.owed)
        .map(|m| (m.user_id, m.owed - m.paid))
        .collect();
    let mut creditors: Vec<(i64, Decimal)> = members
        .iter()
        .filter(|m| m.paid > m.owed)
        .map(|m| (m.user_id, m.paid - m.owed))
        .collect();

    let mut transfers = Vec::new();
    let mut i = 0usize;
    let mut j = 0usize;
    while i < debtors.len() && j < creditors.len() {
        let pay = if debtors[i].1 < creditors[j].1 {
            debtors[i].1
        } else {
            creditors[j].1
        };
        if pay > Decimal::ZERO {
            let from_id = debtors[i].0;
            let to_id = creditors[j].0;
            let from_name = members
                .iter()
                .find(|m| m.user_id == from_id)
                .map(|m| m.nickname.clone())
                .unwrap_or_default();
            let to_name = members
                .iter()
                .find(|m| m.user_id == to_id)
                .map(|m| m.nickname.clone())
                .unwrap_or_default();
            transfers.push(TransferVo {
                from_user_id: from_id,
                from_nickname: from_name,
                to_user_id: to_id,
                to_nickname: to_name,
                amount: dec_to_f64(pay),
            });
        }
        debtors[i].1 -= pay;
        creditors[j].1 -= pay;
        if debtors[i].1 <= Decimal::ZERO {
            i += 1;
        }
        if creditors[j].1 <= Decimal::ZERO {
            j += 1;
        }
    }

    (users, transfers)
}
