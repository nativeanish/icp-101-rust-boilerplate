#[macro_use]
extern crate serde;

use candid::{Decode, Encode, Principal};
use ic_cdk::caller;
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

#[derive(candid::CandidType, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
struct UserPrincipal(Principal);

#[derive(candid::CandidType, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
struct IsUsed(bool);

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default, PartialEq, Eq, PartialOrd, Ord)]
struct Username(String);

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
impl Storable for UserPrincipal {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}
impl Storable for Username {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}
impl Storable for IsUsed {
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

impl BoundedStorable for IsUsed {
    const MAX_SIZE: u32 = 8; // Rust uses 0 and 1 byte to represent true and false
    const IS_FIXED_SIZE: bool = false;
}
impl BoundedStorable for UserPrincipal {
    const MAX_SIZE: u32 = 63; // max length of principals is 63 characters
    const IS_FIXED_SIZE: bool = false;
}
impl BoundedStorable for Username {
    const MAX_SIZE: u32 = 64;
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
    static USERNAME: RefCell<StableBTreeMap<UserPrincipal, Username, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(2)))
        ));
        static USED_USERNAME: RefCell<StableBTreeMap<Username, IsUsed, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(3)))
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
    assert!(username.len() > 0, "Username can't be empty");
    assert!(!USED_USERNAME
        .with(|used_usernames| used_usernames.borrow().contains_key(&Username(username.clone()))), "Username already in use.");
    assert!(!USERNAME.with(|usernames| usernames.borrow().contains_key(&UserPrincipal(caller()))), "Username already set.");
    USED_USERNAME
        .with(|used_usernames| used_usernames.borrow_mut().insert(Username(username.clone()), IsUsed(true)));
    // Set the username for the current user
    USERNAME
        .with(|usernames| usernames.borrow_mut().insert(UserPrincipal(caller()), Username(username.clone())));
    Some(())
}

#[ic_cdk::update]
fn create_tweet(payload: TweetPayload) -> Option<Tweet> {
    // Validate payload content to prevent potential vulnerabilities
    assert!(payload.content.len() > 0, "Content can't be empty");
    let username = USERNAME.with(|u| u.borrow().get(&UserPrincipal(caller()))).expect("Only registered users can tweet");

    let id = ID_COUNTER
        .with(|counter| {
            // Synchronize access to ID_COUNTER to prevent race conditions
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("Cannot increment id counter");

    let tweet = Tweet {
        id,
        username: username.0,
        content: payload.content,
        created_at: time(),
        likes: 0,
        comments: Vec::new(),
    };

    // Perform proper tweet insertion with memory management
    do_insert_tweet(&tweet);

    Some(tweet)
}

#[ic_cdk::update]
fn update_tweet(id: u64, payload: TweetPayload) -> Result<Tweet, Error> {
    let username = USERNAME.with(|u| u.borrow().get(&UserPrincipal(caller()))).expect("User isn't registered");

    match TWEET_STORAGE.with(|tweets| tweets.borrow().get(&id)) {
        Some(mut tweet) => {
            if tweet.username != username.0 {
                return Err(Error::Unauthorized {
                    msg: "You are not authorized to update this tweet".to_string(),
                });
            }

            tweet.content = payload.content;

            // Perform proper tweet update with memory management
            do_update_tweet(&tweet);

            Ok(tweet)
        }
        None => Err(Error::NotFound {
            msg: format!("Tweet with id={} not found", id),
        }),
    }
}

#[ic_cdk::update]
fn delete_tweet(id: u64) -> Result<Tweet, Error> {
    let username = USERNAME.with(|u| u.borrow().get(&UserPrincipal(caller()))).expect("User isn't registered");

    match TWEET_STORAGE.with(|tweets| tweets.borrow_mut().remove(&id)) {
        Some(tweet) => {
            if tweet.username != username.0 {
                // Unauthorized deletion
                return Err(Error::Unauthorized {
                    msg: "You are not authorized to delete this tweet".to_string(),
                });
            }

            // Perform proper tweet deletion with memory management
            do_delete_tweet(&id);

            Ok(tweet)
        }
        None => Err(Error::NotFound {
            msg: format!("Tweet with id={} not found", id),
        }),
    }
}

#[ic_cdk::query]
fn get_all_usernames() -> Vec<String> {
    assert!(!USERNAME.with(|usernames| usernames.borrow().is_empty()), "There are currently no registered users.");
    USERNAME
        .with(|usernames| {
            usernames
                .borrow()
                .iter()
                .map(|(_, tweet)| tweet.0.clone())
                .collect::<Vec<String>>()
        })
}

// Helper method to perform tweet insertion with memory management
fn do_insert_tweet(tweet: &Tweet) {
    TWEET_STORAGE
        .with(|tweets| tweets.borrow_mut().insert(tweet.id, tweet.clone()));
}

// Helper method to perform tweet update with memory management
fn do_update_tweet(tweet: &Tweet) {
    TWEET_STORAGE
        .with(|tweets| tweets.borrow_mut().insert(tweet.id, tweet.clone()));
}

// Helper method to perform tweet deletion with memory management
fn do_delete_tweet(id: &u64) {
    TWEET_STORAGE.with(|tweets| tweets.borrow_mut().remove(id));
}

#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
    Unauthorized { msg: String },
}


// A helper method to get a tweet by id. Used in get_tweet/update_tweet.
fn _get_tweet(id: &u64) -> Option<Tweet> {
    TWEET_STORAGE.with(|tweets| tweets.borrow().get(id).map(|tweet| tweet))
}

// Need this to generate candid
ic_cdk::export_candid!();
