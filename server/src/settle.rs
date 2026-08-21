use rust_decimal::Decimal;
use serde::Serialize;
use std::collections::BTreeMap;

use crate::util::dec_to_f64;

#[derive(Clone)]
pub struct MemberBalance {
    pub user_id: i64,
    pub nickname: String,
    pub avatar: Option<String>,
    pub group_name: Option<String>,
    pub paid: Decimal,
    pub owed: Decimal,
}

#[derive(Serialize)]
pub struct SettleUserVo {
    pub user_id: i64,
    pub nickname: String,
    pub avatar: Option<String>,
    pub group_name: Option<String>,
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

#[derive(Serialize)]
pub struct SettleGroupVo {
    pub group_key: String,
    pub group_name: String,
    /// 是否为命名团体（多人可汇总）；false 表示未分组的个人，一人一组
    pub is_party: bool,
    pub member_count: i64,
    pub paid: f64,
    pub owed: f64,
    pub net: f64,
    pub members: Vec<SettleUserVo>,
    /// 团体内部转账（仅 is_party 且多人时有意义）
    pub inner_transfers: Vec<TransferVo>,
}

#[derive(Serialize)]
pub struct GroupTransferVo {
    pub from_group: String,
    pub to_group: String,
    pub amount: f64,
}

fn party_name(m: &MemberBalance) -> Option<String> {
    m.group_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn display_group(m: &MemberBalance) -> String {
    party_name(m).unwrap_or_else(|| m.nickname.clone())
}

fn group_key(m: &MemberBalance) -> String {
    party_name(m)
        .map(|s| format!("g:{s}"))
        .unwrap_or_else(|| format!("u:{}", m.user_id))
}

fn user_vo(m: &MemberBalance) -> SettleUserVo {
    SettleUserVo {
        user_id: m.user_id,
        nickname: m.nickname.clone(),
        avatar: m.avatar.clone(),
        group_name: party_name(m),
        paid: dec_to_f64(m.paid),
        owed: dec_to_f64(m.owed),
        net: dec_to_f64(m.paid - m.owed),
    }
}

fn transfer_loop(members: &[MemberBalance]) -> Vec<TransferVo> {
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
    transfers
}

pub fn calc_transfers(members: &[MemberBalance]) -> (Vec<SettleUserVo>, Vec<TransferVo>) {
    (members.iter().map(user_vo).collect(), transfer_loop(members))
}

/// 未分组个人 = 一人一组；同名团体汇总为对外结算单位；多人团体另算内部转账
pub fn calc_group_settle(members: &[MemberBalance]) -> (Vec<SettleGroupVo>, Vec<GroupTransferVo>) {
    let mut map: BTreeMap<String, (String, bool, Decimal, Decimal, Vec<MemberBalance>)> =
        BTreeMap::new();
    for m in members {
        let key = group_key(m);
        let name = display_group(m);
        let is_party = party_name(m).is_some();
        let entry = map.entry(key).or_insert_with(|| {
            (name, is_party, Decimal::ZERO, Decimal::ZERO, Vec::new())
        });
        entry.2 += m.paid;
        entry.3 += m.owed;
        entry.4.push(m.clone());
    }

    let groups: Vec<SettleGroupVo> = map
        .iter()
        .map(|(key, (name, is_party, paid, owed, ms))| {
            let inner_transfers = if *is_party && ms.len() > 1 {
                transfer_loop(ms)
            } else {
                Vec::new()
            };
            SettleGroupVo {
                group_key: key.clone(),
                group_name: name.clone(),
                is_party: *is_party,
                member_count: ms.len() as i64,
                paid: dec_to_f64(*paid),
                owed: dec_to_f64(*owed),
                net: dec_to_f64(*paid - *owed),
                members: ms.iter().map(user_vo).collect(),
                inner_transfers,
            }
        })
        .collect();

    let mut debtors: Vec<(String, Decimal)> = map
        .values()
        .filter(|(_, _, paid, owed, _)| paid < owed)
        .map(|(name, _, paid, owed, _)| (name.clone(), *owed - *paid))
        .collect();
    let mut creditors: Vec<(String, Decimal)> = map
        .values()
        .filter(|(_, _, paid, owed, _)| paid > owed)
        .map(|(name, _, paid, owed, _)| (name.clone(), *paid - *owed))
        .collect();

    let mut group_transfers = Vec::new();
    let mut i = 0usize;
    let mut j = 0usize;
    while i < debtors.len() && j < creditors.len() {
        let pay = if debtors[i].1 < creditors[j].1 {
            debtors[i].1
        } else {
            creditors[j].1
        };
        if pay > Decimal::ZERO {
            group_transfers.push(GroupTransferVo {
                from_group: debtors[i].0.clone(),
                to_group: creditors[j].0.clone(),
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

    (groups, group_transfers)
}
