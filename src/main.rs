#[macro_use]
extern crate rocket;

use std::convert::Into;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use ckb_sdk::CkbRpcClient;
use ckb_sdk::rpc::ckb_indexer::{Cell, ScriptSearchMode, SearchKey};
use ckb_sdk::traits::{CellQueryOptions, PrimaryScriptType};
use ckb_types::{h256, H256};
use ckb_types::core::ScriptHashType;
use ckb_types::packed::Script;
use ckb_types::prelude::*;
use molecule::prelude::*;
use rocket::serde::Serialize;
use rocket::State;
use spore_types::generated::spore_types::ClusterData;

// These are testnet code hashes
const SPORE_CODE_HASH: H256 = h256!("0xc1a7e2d2bd7e0fa90e2f1121782aa9f71204d1fee3a634bf3b12c61a69ee574f");
const CLUSTER_CODE_HASH: H256 = h256!("0x598d793defef36e2eeba54a9b45130e4ca92822e1d193671f490950c3b856080");


// utils transfer spore cell into json

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
struct SporeJsonData {
    #[serde(rename(serialize = "content-type"))]
    pub content_type: String,
    pub content: Vec<u8>,
    pub id: String,
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
struct ClusterJsonData {
    pub name: String,
    pub description: String,
    pub id: String,
}

impl From<Cell> for SporeJsonData {
    fn from(cell: Cell) -> Self {
        let data = cell.output_data.unwrap();
        let raw_spore = spore_types::SporeData::from_slice(data.as_bytes()).unwrap();
        SporeJsonData {
            content_type: String::from_utf8(raw_spore.content_type().as_reader().raw_data().to_vec()).unwrap_or_default(),
            content: raw_spore.content().as_bytes().to_vec(),
            id: format!("0x{}", hex_string(cell.output.type_.unwrap_or_default().args.as_bytes())),
        }
    }
}

impl From<Cell> for ClusterJsonData {
    fn from(cell: Cell) -> Self {
        let data = cell.output_data.unwrap();
        println!("{}",format!("0x{}", hex_string(cell.output.type_.clone().unwrap_or_default().args.as_bytes())));
        let raw_cluster = ClusterData::from_slice(data.as_bytes()).unwrap_or_default();
        ClusterJsonData {
            name: String::from_utf8(raw_cluster.name().as_reader().raw_data().to_vec()).unwrap_or_default(),
            description: String::from_utf8(raw_cluster.description().as_reader().raw_data().to_vec()).unwrap_or_default(),
            id: format!("0x{}", hex_string(cell.output.type_.unwrap_or_default().args.as_bytes())),
        }
    }
}

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

struct ClientContext {
    pub client: Arc<Mutex<CkbRpcClient>>,
}

fn get_cells_resp<T>(client: &mut CkbRpcClient, code_hash: H256, id: Option<&str>) -> String where T: Serialize + From<Cell> {
    let mut script_builder = Script::new_builder()
        .code_hash(SPORE_CODE_HASH.pack()).hash_type(ScriptHashType::Data1.into());
    let script = if let Some(id) = id {
        let id = &id[2..];
        let id_h265 = match H256::from_str(id) {
            Ok(id) => id,
            Err(err) => return err.to_string(),
        };
        script_builder.args(id_h265.as_bytes().pack()).build()
    } else {
        script_builder.build()
    };

    let mut query = CellQueryOptions::new(script, PrimaryScriptType::Type);
    query.script_search_mode = Some(ScriptSearchMode::Prefix);
    query.min_total_capacity = u64::MAX;
    let search_key = SearchKey::from(query);
    let mut page = match client.get_cells(search_key.clone(), ckb_sdk::rpc::ckb_light_client::Order::Asc, 10u32.into(), None) {
        Ok(page) => page,
        Err(err) => return serde_json::json!({
            "code": -1,
            "message": err.to_string()
        }).to_string(),
    };
    let mut cells = Vec::new();
    while !page.objects.is_empty() {
        page.objects.clone().into_iter().for_each(|cell| {
            cells.push(T::from(cell));
        });
        page = match client.get_cells(search_key.clone(), ckb_sdk::rpc::ckb_light_client::Order::Asc, 10u32.into(), Some(page.last_cursor)) {
            Ok(page) => page,
            Err(err) => return err.to_string(),
        };
    }
    serde_json::json!(
        {
            "code": 0,
            "result": cells,
        }
    ).to_string()
}

#[get("/api/v1/spore/all")]
fn get_all_spore(client: &State<ClientContext>) -> String {
    rocket::tokio::task::block_in_place(|| {
        let mut client = &mut client.inner().client.lock().unwrap();
        get_cells_resp::<SporeJsonData>(&mut client, SPORE_CODE_HASH, None)
    })
}

#[get("/api/v1/spore/id/<id>")]
fn get_spore_by_id(id: &str, client: &State<ClientContext>) -> String {
    rocket::tokio::task::block_in_place(|| {
        let mut client = &mut client.inner().client.lock().unwrap();
        get_cells_resp::<SporeJsonData>(&mut client, SPORE_CODE_HASH, Some(id))
    })
}


#[get("/api/v1/cluster/all")]
fn get_all_cluster(client: &State<ClientContext>) -> String {
    rocket::tokio::task::block_in_place(|| {
        let mut client = &mut client.inner().client.lock().unwrap();
        get_cells_resp::<ClusterJsonData>(&mut client, CLUSTER_CODE_HASH, None)
    })
}

#[get("/api/v1/cluster/id/<id>")]
fn get_cluster_by_id(id: &str, client: &State<ClientContext>) -> String {
    rocket::tokio::task::block_in_place(|| {
        let mut client = &mut client.inner().client.lock().unwrap();
        get_cells_resp::<ClusterJsonData>(&mut client, CLUSTER_CODE_HASH, Some(id))
    })
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .manage(ClientContext { client: Arc::new(Mutex::new(CkbRpcClient::new("https://testnet.ckb.dev"))) }) // replace the PRC URL with your local one to speed up quering
        .mount("/", routes![index, get_all_spore, get_spore_by_id, get_all_cluster, get_cluster_by_id])
}
