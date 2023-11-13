#[macro_use]
extern crate serde;

use candid::{Decode, Encode};
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct Tweet {
    id: u64,
    username: String,
    content: String,
    created_at: u64,
    likes: u64,
    comments: Vec<String>,
}

impl Storable for Tweet {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for Tweet {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    static ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create a counter")
    );

    static TWEET_STORAGE: RefCell<StableBTreeMap<u64, Tweet, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1)))
        ));
}

#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct TweetPayload {
    content: String,
}

#[ic_cdk::query]
fn get_tweet(id: u64) -> Result<Tweet, Error> {
    match _get_tweet(&id) {
        Some(tweet) => Ok(tweet),
        None => Err(Error::NotFound {
            msg: format!("Tweet with id={} not found", id),
        }),
    }
}

#[ic_cdk::update]
fn set_username(username: String) -> Option<()> {
    // Set the username for the current user
    USERNAME.with(|u| u.replace(username));
    Some(())
}

#[ic_cdk::update]
fn create_tweet(payload: TweetPayload) -> Option<Tweet> {
    let id = ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("Cannot increment id counter");

    let username = USERNAME.with(|u| u.borrow().clone());

    let tweet = Tweet {
        id,
        username,
        content: payload.content,
        created_at: time(),
        likes: 0,
        comments: Vec::new(),
    };

    do_insert_tweet(&tweet);
    Some(tweet)
}

#[ic_cdk::update]
fn update_tweet(id: u64, payload: TweetPayload) -> Result<Tweet, Error> {
    let username = USERNAME.with(|u| u.borrow().clone());

    match TWEET_STORAGE.with(|tweets| tweets.borrow().get(&id)) {
        Some(mut tweet) => {
            if tweet.username != username {
                return Err(Error::Unauthorized {
                    msg: "You are not authorized to update this tweet".to_string(),
                });
            }

            tweet.content = payload.content;
            do_insert_tweet(&tweet);
            Ok(tweet)
        }
        None => Err(Error::NotFound {
            msg: format!("Tweet with id={} not found", id),
        }),
    }
}

#[ic_cdk::update]
fn delete_tweet(id: u64) -> Result<Tweet, Error> {
    let username = USERNAME.with(|u| u.borrow().clone());

    match TWEET_STORAGE.with(|tweets| tweets.borrow_mut().remove(&id)) {
        Some(tweet) => {
            if tweet.username != username {
                return Err(Error::Unauthorized {
                    msg: "You are not authorized to delete this tweet".to_string(),
                });
            }
            Ok(tweet)
        }
        None => Err(Error::NotFound {
            msg: format!("Tweet with id={} not found", id),
        }),
    }
}

#[ic_cdk::query]
fn get_all_usernames() -> Vec<String> {
    TWEET_STORAGE
        .with(|tweets| {
            tweets
                .borrow()
                .iter()
                .map(|(_, tweet)| tweet.username.clone())
                .collect::<Vec<String>>()
        })
}

// Helper method to perform tweet insertion.
fn do_insert_tweet(tweet: &Tweet) {
    TWEET_STORAGE
        .with(|tweets| tweets.borrow_mut().insert(tweet.id, tweet.clone()));
}

#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
    Unauthorized { msg: String },
}

thread_local! {
    static USERNAME: RefCell<String> = RefCell::new(String::new());
}

// A helper method to get a tweet by id. Used in get_tweet/update_tweet.
fn _get_tweet(id: &u64) -> Option<Tweet> {
    TWEET_STORAGE.with(|tweets| tweets.borrow().get(id).map(|tweet| tweet))
}

// Need this to generate candid
ic_cdk::export_candid!();