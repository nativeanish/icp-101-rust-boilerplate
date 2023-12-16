#[macro_use]
extern crate serde;

use candid::{Decode, Encode};
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, Storable};
use std::{borrow::Cow, cell::RefCell};
use std::collections::BTreeMap;
type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct Tweet {
    id: u64,
    user_id: u64,
    content: String,
    created_at: u64,
    updated_at: Option<u64>,
}

impl Storable for Tweet {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for Tweet {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct User {
    id: u64,
    username: String,
    bio: String,
}

impl Storable for User {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for User {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    static TWEET_ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create a tweet counter")
    );

    static USER_ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1))), 0)
            .expect("Cannot create a user counter")
    );

    static TWEET_STORAGE: RefCell<BTreeMap<u64, Tweet>> =
    RefCell::new(BTreeMap::new());


        static USER_STORAGE: RefCell<BTreeMap<u64, User>> = RefCell::new(BTreeMap::new());
}

#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct TweetPayload {
    user_id: u64,
    content: String,
}

#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct UserPayload {
    username: String,
    bio: String,
}

#[ic_cdk::query]
fn get_tweet(tweet_id: u64) -> Result<Tweet, Error> {
    let result = TWEET_STORAGE.with(|storage| {
        let borrow = storage.borrow();
        if let Some(tweet) = borrow.get(&tweet_id).cloned() {
            Ok(tweet)
        } else {
            Err(Error::NotFound {
                msg: format!("Tweet with id={} not found", tweet_id),
            })
        }
    });

    match result {
        Ok(tweet) => Ok(tweet),
        Err(err) => Err(err),
    }
}

#[ic_cdk::update]
fn delete_tweet(tweet_id: u64) -> Result<(), Error> {
    let result = TWEET_STORAGE.with(|storage| {
        let mut borrow = storage.borrow_mut();
        if borrow.contains_key(&tweet_id) {
            borrow.remove(&tweet_id);
            Ok(())
        } else {
            Err(Error::NotFound {
                msg: format!("Tweet with id={} not found", tweet_id),
            })
        }
    });

    match result {
        Ok(_) => Ok(()),
        Err(err) => Err(err),
    }
}


#[ic_cdk::update]
fn create_tweet(payload: TweetPayload) -> Option<Tweet> {
    let tweet_id = TWEET_ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("Cannot increment tweet id counter");

    let tweet = Tweet {
        id: tweet_id,
        user_id: payload.user_id,
        content: payload.content,
        created_at: time(),
        updated_at: None,
    };

    TWEET_STORAGE.with(|storage| storage.borrow_mut().insert(tweet_id, tweet.clone()));
    Some(tweet)
}

#[ic_cdk::update]
fn update_tweet(tweet_id: u64, payload: TweetPayload) -> Result<Tweet, Error> {
    let result = TWEET_STORAGE.with(|storage| {
        let mut borrow = storage.borrow_mut();
        if let Some(mut tweet) = borrow.get_mut(&tweet_id).cloned() {
            tweet.user_id = payload.user_id;
            tweet.content = payload.content;
            tweet.updated_at = Some(time());

            Ok(tweet)
        } else {
            Err(Error::NotFound {
                msg: format!("Tweet with id={} not found", tweet_id),
            })
        }
    });

    match result {
        Ok(updated_tweet) => {
            TWEET_STORAGE.with(|storage| {
                storage.borrow_mut().insert(tweet_id, updated_tweet.clone());
            });
            Ok(updated_tweet)
        }
        Err(err) => Err(err),
    }
}

fn _get_tweet(tweet_id: &u64) -> Option<Tweet> {
    TWEET_STORAGE.with(|storage| storage.borrow().get(tweet_id).cloned())
}


#[ic_cdk::update]
fn create_user(payload: UserPayload) -> Option<User> {
    let user_id = USER_ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("Cannot increment user id counter");

    let user = User {
        id: user_id,
        username: payload.username,
        bio: payload.bio,
    };

    USER_STORAGE.with(|storage| storage.borrow_mut().insert(user_id, user.clone()));
    Some(user)
}

#[ic_cdk::update]
fn edit_user(user_id: u64, payload: UserPayload) -> Result<User, Error> {
    match USER_STORAGE.with(|storage| {
        let mut borrow = storage.borrow_mut();
        match borrow.get_mut(&user_id) {
            Some(user) => {
                user.username = payload.username;
                user.bio = payload.bio;
                Ok(user.clone())
            }
            None => Err(Error::NotFound {
                msg: format!("User with id={} not found", user_id),
            }),
        }
    }) {
        Ok(result) => Ok(result),
        Err(err) => Err(err),
    }
}


#[ic_cdk::query]
fn get_user(user_id: u64) -> Result<User, Error> {
    match USER_STORAGE.with(|storage| {
        let borrow = storage.borrow();
        if let Some(user) = borrow.get(&user_id).cloned() {
            Ok(user)
        } else {
            Err(Error::NotFound {
                msg: format!("User with id={} not found", user_id),
            })
        }
    }) {
        Ok(result) => Ok(result),
        Err(err) => Err(err),
    }
}

#[ic_cdk::query]
fn get_all_users() -> Vec<User> {
    USER_STORAGE.with(|storage| storage.borrow().values().cloned().collect())
}

#[ic_cdk::update]
fn delete_user(user_id: u64) -> Result<(), Error> {
    match USER_STORAGE.with(|storage| {
        let mut borrow = storage.borrow_mut();
        if borrow.contains_key(&user_id) {
            borrow.remove(&user_id);
            Ok(())
        } else {
            Err(Error::NotFound {
                msg: format!("User with id={} not found", user_id),
            })
        }
    }) {
        Ok(result) => Ok(result),
        Err(err) => Err(err),
    }
}

#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
}

ic_cdk::export_candid!();