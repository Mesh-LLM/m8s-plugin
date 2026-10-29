//! Fresh read-only account inventory and on-demand price qualification.
//! Refuses every existing instance/volume; no adoption, mutation or credential
//! discovery. Public normalized facts only. Prices are observations, not invoices.
use super::*;
use std::num::NonZeroU64;
#[derive(serde::Serialize)]
pub struct DemoPreflight {
    pub account_id: u64,
    pub observed_at: i64,
    pub offer_id: NonZeroU64,
    pub compute_micro_usd_per_hour: u64,
    pub storage_micro_usd_per_hour: u64,
    pub transfer_micro_usd_per_gb: u64,
    pub available_credit_micro_usd: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_reported_gpu: Option<GpuObservation>,
}
#[derive(serde::Serialize)]
pub struct GpuObservation {
    pub name: String,
    pub memory_mib: u64,
    pub host_memory_mib: u64,
    pub advertised_download_mbps: u64,
    pub driver_cuda_support: f64,
}
/// Explicit experiment sizing. Prices still enter the durable budget admission.
#[derive(Clone, Copy)]
pub struct Requirements {
    pub max_compute_hourly: u64,
    pub max_transfer_micro_usd_per_gb: u64,
    pub min_gpu_memory_mib: u64,
    pub min_host_memory_mib: u64,
    pub min_download_mbps: u64,
    pub blackwell_sm120: bool,
}
impl Default for Requirements {
    fn default() -> Self {
        Self {
            max_compute_hourly: 300_000,
            max_transfer_micro_usd_per_gb: 100_000,
            min_gpu_memory_mib: 0,
            min_host_memory_mib: 8192,
            min_download_mbps: 0,
            blackwell_sm120: false,
        }
    }
}
/// Sanitized, read-only catalog. A listing is not launch authorization.
#[derive(serde::Serialize)]
pub struct OfferCatalog {
    pub account_id: u64,
    pub observed_at: i64,
    pub disk_gb: u32,
    pub offers: Vec<CatalogOffer>,
}
#[derive(serde::Serialize)]
pub struct CatalogOffer {
    pub machine_id: NonZeroU64,
    pub qualified_for_profile: bool,
    pub id: NonZeroU64,
    pub gpu_name: String,
    pub gpu_count: u64,
    pub gpu_memory_mib: u64,
    pub host_memory_mib: u64,
    pub location: String,
    pub compute_micro_usd_per_hour: u64,
    pub storage_micro_usd_per_hour: u64,
    pub transfer_micro_usd_per_gb: u64,
}
fn catalog_offer(row: &Value, disk: u32) -> Result<CatalogOffer> {
    let requirements = Requirements {
        max_compute_hourly: 3_000_000,
        ..Requirements::default()
    };
    let (id, compute, storage, transfer) = qualify(row, disk, requirements)?;
    let text = |key| {
        row.get(key)
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty() && v.len() <= 160 && !v.chars().any(char::is_control))
            .map(str::to_owned)
            .ok_or(Error::Unavailable)
    };
    let gpu_count = number(row, "num_gpus", 1, false)?;
    let gpu_memory_mib = number(row, "gpu_ram", 1, false)?;
    if !(1..=16).contains(&gpu_count) || gpu_memory_mib == 0 || gpu_memory_mib > 1024 * 1024 {
        return Err(Error::Unavailable);
    }
    Ok(CatalogOffer {
        machine_id: row
            .get("machine_id")
            .and_then(Value::as_u64)
            .and_then(NonZeroU64::new)
            .ok_or(Error::Unavailable)?,
        qualified_for_profile: false,
        id,
        gpu_name: text("gpu_name")?,
        gpu_count,
        gpu_memory_mib,
        host_memory_mib: number(row, "cpu_ram", 1, false)?,
        location: text("geolocation")?,
        compute_micro_usd_per_hour: compute,
        storage_micro_usd_per_hour: storage,
        transfer_micro_usd_per_gb: transfer,
    })
}
pub struct PreflightHttps<S> {
    transport: ReadonlyHttps<S>,
    stage: &'static str,
}
impl<S: FnMut() -> Result<SecretValue>> PreflightHttps<S> {
    pub fn new(supplier: S) -> Result<Self> {
        Ok(Self {
            transport: ReadonlyHttps::new(supplier)?,
            stage: "vast-client-initialization",
        })
    }
    pub fn last_stage(&self) -> &'static str {
        self.stage
    }
    pub fn transport_failure(&self) -> Option<&'static str> {
        self.transport.backend.1
    }
    fn read(&mut self, path: &str, body: Option<Value>) -> Result<Value> {
        read_with(
            &mut self.transport.backend,
            &mut self.transport.supplier,
            path,
            body,
        )
    }
    /// Authenticate the expected account only. Cleanup must remain possible with
    /// zero/negative credit and existing resources; no inventory or offer read.
    /// The caller must retain the same credential for subsequent resource reads.
    pub fn authenticate_account(&mut self, expected: NonZeroU64) -> Result<()> {
        self.stage = "vast-account-identity";
        authenticate_account_with(
            &mut self.transport.backend,
            &mut self.transport.supplier,
            expected,
        )
    }
    pub fn browse(&mut self, disk_gb: u32, requirements: Requirements) -> Result<OfferCatalog> {
        if !(1..=128).contains(&disk_gb) {
            return Err(Error::Invalid);
        }
        self.stage = "vast-account-read";
        let account = self.read("/api/v0/users/current/", None)?;
        let account_id = account
            .get("id")
            .and_then(Value::as_u64)
            .filter(|id| *id > 0)
            .ok_or(Error::Unavailable)?;
        self.stage = "vast-offer-read";
        // Include a bounded sample of size classes and the qualified runtime
        // profile, rather than burying compatible cards under cheap listings.
        let mut offers = Vec::new();
        for minimum in [0, 24000, 48000, 80000, u64::MAX] {
            let mut query = json!({"allocated_storage":disk_gb,"limit":15,"type":"on-demand",
                "verified":{"eq":true},"rentable":{"eq":true},"rented":{"eq":false},
                "gpu_ram":{"gte":minimum},"disk_space":{"gte":disk_gb},"dph_total":{"lte":3},"order":[["dph_total","asc"]]});
            if minimum == u64::MAX {
                query["gpu_ram"] = json!({"gte":requirements.min_gpu_memory_mib});
                query["num_gpus"] = json!({"eq":1});
                query["cpu_cores_effective"] = json!({"gte":4});
                query["geolocation"] = json!({"notin": ["CN", "CU", "IR", "KP", "SY", "RU", "BY"]});
                query["cpu_ram"] = json!({"gte":requirements.min_host_memory_mib});
                query["inet_down"] = json!({"gte":requirements.min_download_mbps});
                query["direct_port_count"] = json!({"gte":2});
                for direction in ["inet_up_cost", "inet_down_cost"] {
                    query[direction] = json!({"lte":requirements.max_transfer_micro_usd_per_gb as f64/1_000_000.0});
                }
                if requirements.blackwell_sm120 {
                    query["gpu_name"] = json!({"in":["RTX PRO 5000","RTX PRO 6000","RTX PRO 6000 WS","RTX PRO 6000 S"]});
                    query["cuda_max_good"] = json!({"gte":12.8});
                }
            }
            let value = self.read("/api/v0/bundles/", Some(query))?;
            let rows = value
                .get("offers")
                .and_then(Value::as_array)
                .filter(|rows| rows.len() <= 15)
                .ok_or(Error::Unavailable)?;
            for row in rows {
                if let Ok(mut offer) = catalog_offer(row, disk_gb) {
                    offer.qualified_for_profile =
                        minimum == u64::MAX && qualify(row, disk_gb, requirements).is_ok();
                    if let Some(existing) = offers
                        .iter_mut()
                        .find(|existing: &&mut CatalogOffer| existing.id == offer.id)
                    {
                        existing.qualified_for_profile |= offer.qualified_for_profile;
                    } else {
                        offers.push(offer);
                    }
                }
            }
        }
        offers.sort_by_key(|offer| {
            (
                !offer.qualified_for_profile,
                offer.compute_micro_usd_per_hour + offer.storage_micro_usd_per_hour,
            )
        });
        let observed_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|time| i64::try_from(time.as_secs()).ok())
            .ok_or(Error::Unavailable)?;
        Ok(OfferCatalog {
            account_id,
            observed_at,
            disk_gb,
            offers,
        })
    }
    pub fn observe(
        &mut self,
        expected_account: u64,
        selected_offer: Option<NonZeroU64>,
        disk_gb: u32,
    ) -> Result<DemoPreflight> {
        self.observe_sized(
            expected_account,
            selected_offer,
            disk_gb,
            Requirements::default(),
        )
    }
    pub fn observe_sized(
        &mut self,
        expected_account: u64,
        selected_offer: Option<NonZeroU64>,
        disk_gb: u32,
        requirements: Requirements,
    ) -> Result<DemoPreflight> {
        self.observe_on_machine(
            expected_account,
            selected_offer,
            disk_gb,
            requirements,
            None,
        )
    }
    pub fn observe_on_machine(
        &mut self,
        expected_account: u64,
        selected_offer: Option<NonZeroU64>,
        disk_gb: u32,
        requirements: Requirements,
        machine_id: Option<NonZeroU64>,
    ) -> Result<DemoPreflight> {
        self.stage = "vast-sizing-validation";
        if expected_account == 0
            || !(1..=128).contains(&disk_gb)
            || requirements.max_compute_hourly == 0
            || requirements.max_compute_hourly > 3_000_000
            || requirements.max_transfer_micro_usd_per_gb > 100_000
            || requirements.min_gpu_memory_mib > 192 * 1024
            || requirements.min_download_mbps > 100_000
        {
            return Err(Error::Invalid);
        }
        self.stage = "vast-account-read";
        let account = self.read("/api/v0/users/current/", None)?;
        self.stage = "vast-account-identity";
        let account_id = account
            .get("id")
            .and_then(Value::as_u64)
            .ok_or(Error::Unavailable)?;
        if account_id != expected_account {
            return Err(Error::Unavailable);
        }
        self.stage = "vast-credit-check";
        let credit = number(&account, "credit", 1_000_000, false)?;
        if credit < 6_000_000 {
            return Err(Error::Unavailable);
        }
        self.stage = "vast-complete-inventory";
        let inventory = self.read(
            "/api/v1/instances/?limit=1&select_cols=%5B%22id%22%5D",
            None,
        )?;
        if inventory.get("success") != Some(&Value::Bool(true))
            || inventory
                .get("instances")
                .and_then(Value::as_array)
                .is_none_or(|v| !v.is_empty())
            || inventory.get("next_token") != Some(&Value::Null)
            || inventory.get("instances_found").and_then(Value::as_u64) != Some(0)
            || inventory.get("total_instances").and_then(Value::as_u64) != Some(0)
        {
            return Err(Error::Unavailable);
        }
        self.stage = "vast-volume-inventory";
        let volumes = self.read("/api/v0/volumes/?owner=me&type=all_volume", None)?;
        if volumes
            .get("volumes")
            .and_then(Value::as_array)
            .is_none_or(|v| !v.is_empty())
        {
            return Err(Error::Unavailable);
        }
        let mut query = json!({"allocated_storage":disk_gb,"limit":offer_read_limit(selected_offer),"type":"on-demand","verified":{"eq":true},"rentable":{"eq":true},"rented":{"eq":false},"cpu_cores_effective":{"gte":4},"cpu_ram":{"gte":8192},"disk_space":{"gte":disk_gb},"direct_port_count":{"gte":2},"dph_total":{"lte": requirements.max_compute_hourly as f64 / 1_000_000.0},"order":[["dph_total","asc"]]});
        for direction in ["inet_up_cost", "inet_down_cost"] {
            query[direction] =
                json!({"lte": requirements.max_transfer_micro_usd_per_gb as f64 / 1_000_000.0});
        }
        if requirements.min_gpu_memory_mib > 0 {
            query["gpu_ram"] = json!({"gte": requirements.min_gpu_memory_mib});
            query["num_gpus"] = json!({"eq": 1});
            query["cpu_ram"] = json!({"gte": requirements.min_host_memory_mib});
            query["inet_down"] = json!({"gte": requirements.min_download_mbps});
            query["geolocation"] = json!({"notin": ["CN", "CU", "IR", "KP", "SY", "RU", "BY"]});
        }
        if requirements.blackwell_sm120 {
            query["gpu_name"] = json!({"in": ["RTX PRO 5000", "RTX PRO 6000", "RTX PRO 6000 WS", "RTX PRO 6000 S"]});
            query["cuda_max_good"] = json!({"gte": 12.8});
        }
        if let Some(machine) = machine_id {
            query["machine_id"] = json!({"eq":machine.get()});
        }
        // The API's `id` search filter does not address the returned bundle ID:
        // live observation returned no rows for an ID present in the same host
        // search. Keep a bounded catalog read and match the exact bundle locally.
        // Never substitute a different offer when a selected one is unavailable.
        self.stage = "vast-offer-read";
        let offers = self.read("/api/v0/bundles/", Some(query))?;
        self.stage = "vast-offer-price-qualification";
        let rows = offers
            .get("offers")
            .and_then(Value::as_array)
            .filter(|r| r.len() <= offer_read_limit(selected_offer))
            .ok_or(Error::Unavailable)?;
        let mut candidates = Vec::new();
        for row in rows {
            if machine_id.is_some_and(|machine| {
                row.get("machine_id").and_then(Value::as_u64) != Some(machine.get())
            }) {
                continue;
            }
            if let Ok(candidate) = qualify(row, disk_gb, requirements) {
                candidates.push(candidate);
            }
        }
        candidates.sort_by_key(|v| v.1 + v.2);
        let (offer_id, compute, storage, transfer) = candidates
            .into_iter()
            .find(|v| selected_offer.is_none_or(|id| id == v.0))
            .ok_or(Error::Unavailable)?;
        let provider_reported_gpu = if requirements.blackwell_sm120 {
            let row = rows
                .iter()
                .find(|row| row.get("id").and_then(Value::as_u64) == Some(offer_id.get()))
                .ok_or(Error::Unavailable)?;
            Some(GpuObservation {
                name: row["gpu_name"].as_str().ok_or(Error::Unavailable)?.into(),
                memory_mib: number(row, "gpu_ram", 1, false)?,
                host_memory_mib: number(row, "cpu_ram", 1, false)?,
                advertised_download_mbps: number(row, "inet_down", 1, false)?,
                driver_cuda_support: row["cuda_max_good"].as_f64().ok_or(Error::Unavailable)?,
            })
        } else {
            None
        };
        let observed_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|v| i64::try_from(v.as_secs()).ok())
            .ok_or(Error::Unavailable)?;
        Ok(DemoPreflight {
            account_id,
            observed_at,
            offer_id,
            compute_micro_usd_per_hour: compute,
            storage_micro_usd_per_hour: storage,
            transfer_micro_usd_per_gb: transfer,
            available_credit_micro_usd: credit,
            provider_reported_gpu,
        })
    }
}
fn read_with<B: Backend, S: FnMut() -> Result<SecretValue>>(
    backend: &mut B,
    supplier: &mut S,
    path: &str,
    body: Option<Value>,
) -> Result<Value> {
    let secret = supplier()?;
    let bytes = secret.expose();
    if bytes.is_empty() || bytes.len() > 4096 || !bytes.iter().all(|b| (33..=126).contains(b)) {
        return Err(Error::Unavailable);
    }
    let bearer = SecretValue::new([b"Bearer ".as_slice(), bytes].concat());
    let mut authorization =
        HeaderValue::from_bytes(bearer.expose()).map_err(|_| Error::Unavailable)?;
    authorization.set_sensitive(true);
    let method = if body.is_some() {
        reqwest::Method::POST
    } else {
        reqwest::Method::GET
    };
    let reply = backend
        .send(WireRequest {
            method,
            url: format!("{ORIGIN}{path}"),
            authorization,
            body: body
                .map(|v| serde_json::to_vec(&v).map_err(|_| Error::Unavailable))
                .transpose()?,
        })
        .map_err(|_| Error::Unavailable)?;
    checked_json(reply, bytes, 256 * 1024)
}

pub(super) fn authenticate_account_with<B: Backend, S: FnMut() -> Result<SecretValue>>(
    backend: &mut B,
    supplier: &mut S,
    expected: NonZeroU64,
) -> Result<()> {
    let account = read_with(backend, supplier, "/api/v0/users/current/", None)?;
    if account.get("id").and_then(Value::as_u64) != Some(expected.get()) {
        return Err(Error::Unavailable);
    }
    Ok(())
}

fn offer_read_limit(selected: Option<NonZeroU64>) -> usize {
    if selected.is_some() { 100 } else { 20 }
}

// Decimal dollars -> micro-USD, always round costs up. Reject exponent syntax,
// negative values, overflow and non-finite provider values.
fn micros(v: &Value) -> Result<u64> {
    let text = if let Some(s) = v.as_str() {
        s.to_owned()
    } else {
        v.to_string()
    };
    let (whole, fraction) = text.split_once('.').unwrap_or((&text, ""));
    if whole.is_empty()
        || whole.len() > 9
        || fraction.len() > 32
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(Error::Unavailable);
    }
    let whole: u64 = whole.parse().map_err(|_| Error::Unavailable)?;
    let mut frac = fraction.chars().take(6).collect::<String>();
    while frac.len() < 6 {
        frac.push('0');
    }
    let extra = u64::from(fraction.bytes().skip(6).any(|b| b != b'0'));
    whole
        .checked_mul(1_000_000)
        .and_then(|n| n.checked_add(frac.parse::<u64>().ok()?))
        .and_then(|n| n.checked_add(extra))
        .ok_or(Error::Unavailable)
}
fn qualify(
    row: &Value,
    disk: u32,
    requirements: Requirements,
) -> Result<(NonZeroU64, u64, u64, u64)> {
    let get = |key| row.get(key).ok_or(Error::Unavailable);
    let id = get("id")?
        .as_u64()
        .and_then(NonZeroU64::new)
        .ok_or(Error::Unavailable)?;
    let compute = micros(get("dph_total")?)?;
    // Search Offers explicitly defines storage_cost as USD/GB/month:
    // https://docs.vast.ai/api-reference/search/search-offers
    // Normalize with the shortest calendar month (28 days), rounding UP.
    // dph_total may already include disk; adding this reserve double-counts
    // that component conservatively rather than silently excluding storage.
    let monthly_storage = micros(get("storage_cost")?)?
        .checked_mul(disk.into())
        .ok_or(Error::Unavailable)?;
    let storage = monthly_storage.div_ceil(28 * 24);
    let transfer = micros(get("inet_up_cost")?)?.max(micros(get("inet_down_cost")?)?);
    if compute == 0
        || compute > requirements.max_compute_hourly
        || storage > 32_000_000
        || transfer > requirements.max_transfer_micro_usd_per_gb
        || get("rentable")?.as_bool() != Some(true)
        || get("rented")?.as_bool() != Some(false)
        || row.get("verification").and_then(Value::as_str) != Some("verified")
    {
        return Err(Error::Unavailable);
    }
    if requirements.min_gpu_memory_mib > 0 {
        if number(row, "gpu_ram", 1, false)? < requirements.min_gpu_memory_mib
            || number(row, "num_gpus", 1, false)? != 1
            || number(row, "cpu_ram", 1, false)? < requirements.min_host_memory_mib
            || number(row, "inet_down", 1, false)? < requirements.min_download_mbps
        {
            return Err(Error::Unavailable);
        }
        let country = row
            .get("geolocation")
            .and_then(Value::as_str)
            .and_then(|s| s.rsplit(',').next())
            .map(str::trim)
            .ok_or(Error::Unavailable)?;
        // Bound the live experiment to explicitly supported host locations.
        if !matches!(country, "US" | "NL" | "CZ" | "FR") {
            return Err(Error::Unavailable);
        }
    }
    if requirements.blackwell_sm120
        && (!matches!(
            row.get("gpu_name").and_then(Value::as_str),
            Some("RTX PRO 5000" | "RTX PRO 6000" | "RTX PRO 6000 WS" | "RTX PRO 6000 S")
        ) || !row
            .get("cuda_max_good")
            .and_then(Value::as_f64)
            .is_some_and(|n| n >= 12.8))
    {
        return Err(Error::Unavailable);
    }
    Ok((id, compute, storage, transfer))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleanup_account_authentication_reads_only_identity_not_credit_or_inventory() {
        use super::super::tests::{Fake, key};
        for body in [
            br#"{"id":7}"#.as_slice(),
            br#"{"id":7,"credit":-10,"balance":-10}"#,
        ] {
            let mut fake = Fake::json(body);
            authenticate_account_with(&mut fake, &mut key, NonZeroU64::new(7).unwrap()).unwrap();
            assert_eq!(fake.calls, 1);
            let sent = fake.request.unwrap();
            assert_eq!(sent.method, reqwest::Method::GET);
            assert_eq!(sent.url, "https://console.vast.ai/api/v0/users/current/");
            assert!(sent.body.is_none());
            assert!(sent.authorization.is_sensitive());
        }
    }

    #[test]
    fn cleanup_account_mismatch_missing_ambiguous_or_failed_identity_refuses() {
        use super::super::tests::{Fake, key};
        for body in [
            br#"{}"#.as_slice(),
            br#"{"id":8}"#,
            br#"{"id":"7"}"#,
            br#"{"id":0}"#,
            br#"{"id":7,"id":8}"#,
            br#"{"id":7,"success":false}"#,
            br#"{"id":7,"error":"forbidden"}"#,
        ] {
            let mut fake = Fake::json(body);
            assert!(
                authenticate_account_with(&mut fake, &mut key, NonZeroU64::new(7).unwrap())
                    .is_err()
            );
            assert_eq!(fake.calls, 1);
        }
        for status in [301, 401, 403, 404, 429, 500] {
            let mut fake = Fake::json(br#"{"id":7}"#);
            fake.reply.as_mut().unwrap().status = status;
            assert!(
                authenticate_account_with(&mut fake, &mut key, NonZeroU64::new(7).unwrap())
                    .is_err()
            );
        }
    }

    #[test]
    fn gpu_offer_is_rechecked_and_monthly_storage_rounds_up_to_hourly() {
        let requirements = Requirements {
            max_compute_hourly: 3_000_000,
            max_transfer_micro_usd_per_gb: 4000,
            min_gpu_memory_mib: 48000,
            min_host_memory_mib: 64000,
            min_download_mbps: 4000,
            blackwell_sm120: true,
        };
        let row = json!({"id":42,"dph_total":"0.74","storage_cost":"0.2",
            "inet_up_cost":"0.004","inet_down_cost":"0.003", "rentable":true,
            "rented":false,"verification":"verified","gpu_ram":48935,"num_gpus":1,
            "cpu_ram":64000,"inet_down":12000,"geolocation":"Virginia, US",
            "gpu_name":"RTX PRO 5000","cuda_max_good":13.0});
        let qwen = Requirements {
            min_host_memory_mib: 128000,
            ..requirements
        };
        assert!(qualify(&row, 80, qwen).is_err());
        let mut larger_host = row.clone();
        larger_host["cpu_ram"] = json!(128000);
        assert!(qualify(&larger_host, 80, qwen).is_ok());
        larger_host["cpu_ram"] = json!(127999);
        assert!(qualify(&larger_host, 80, qwen).is_err());
        let (_, compute, storage, transfer) = qualify(&row, 80, requirements).unwrap();
        assert_eq!((compute, storage, transfer), (740000, 23810, 4000));
        // A sub-micro hourly fraction must still reserve one whole micro-USD.
        let mut fractional = row.clone();
        fractional["storage_cost"] = json!("0.000001");
        assert_eq!(qualify(&fractional, 1, requirements).unwrap().2, 1);
        fractional["storage_cost"] = json!("0");
        assert_eq!(qualify(&fractional, 1, requirements).unwrap().2, 0);
        for invalid in [json!(-1), json!(true), Value::Null] {
            fractional["storage_cost"] = invalid;
            assert!(qualify(&fractional, 1, requirements).is_err());
        }
        assert!(qualify(&row, 80, Requirements::default()).is_err());
        for location in ["Netherlands, NL", "Czechia, CZ", "France, FR"] {
            let mut fast_host = row.clone();
            fast_host["geolocation"] = json!(location);
            assert!(qualify(&fast_host, 80, requirements).is_ok());
        }
        for (key, value) in [
            ("gpu_ram", json!(47000)),
            ("gpu_name", json!("RTX 6000Ada")),
            ("cuda_max_good", json!(12.7)),
            ("num_gpus", json!(2)),
            ("cpu_ram", json!(32000)),
            ("inet_down", json!(3999)),
            ("geolocation", json!("China, CN")),
            ("dph_total", json!("3.000001")),
            ("inet_up_cost", json!("0.004000001")),
            ("inet_down_cost", json!("0.004000001")),
            ("rented", json!(true)),
        ] {
            let mut invalid = row.clone();
            invalid[key] = value;
            assert!(qualify(&invalid, 80, requirements).is_err(), "{key}");
        }
    }
    #[test]
    fn selected_offer_catalog_read_remains_bounded() {
        assert_eq!(offer_read_limit(None), 20);
        assert_eq!(offer_read_limit(NonZeroU64::new(42)), 100);
    }
    #[test]
    fn decimal_cost_rounds_up_without_float_arithmetic() {
        for (s, n) in [
            ("0", 0),
            ("0.0000001", 1),
            ("0.125", 125000),
            ("6.24524799", 6245248),
            ("0.0026041666666666665", 2605),
        ] {
            assert_eq!(micros(&json!(s)).unwrap(), n);
        }
        // Actual provider JSON numbers can contain more than 18 fractional
        // digits. Keep integer rounding and a bounded input representation.
        assert_eq!(micros(&json!(0.0026041666666666665)).unwrap(), 2605);
        assert!(micros(&json!(format!("0.{}", "1".repeat(33)))).is_err());
        for s in ["-1", "NaN", "1e-4", "1.2.3", ""] {
            assert!(micros(&json!(s)).is_err());
        }
    }
}

#[cfg(test)]
mod catalog_tests {
    use super::*;
    #[test]
    fn catalog_does_not_pool_gpu_memory_or_expose_provider_secrets() {
        let row = json!({"id":7,"machine_id":8,"dph_total":0.25,"storage_cost":0.10,"inet_up_cost":0.01,"inet_down_cost":0.02,"rentable":true,"rented":false,"verification":"verified","num_gpus":2,"gpu_ram":24576,"cpu_ram":131072,"gpu_name":"Example GPU","geolocation":"US","secret":"do-not-project"});
        let offer = catalog_offer(&row, 80).unwrap();
        assert_eq!(offer.gpu_memory_mib, 24576);
        assert_eq!(offer.gpu_count, 2);
        let serialized = serde_json::to_string(&offer).unwrap();
        assert!(!serialized.contains("do-not-project"));
        for (key, value) in [
            ("rentable", json!(false)),
            ("verification", json!("unverified")),
            ("num_gpus", json!(0)),
            ("gpu_ram", json!(0)),
            ("gpu_name", json!("bad\nname")),
        ] {
            let mut bad = row.clone();
            bad[key] = value;
            assert!(catalog_offer(&bad, 80).is_err());
        }
    }
}
