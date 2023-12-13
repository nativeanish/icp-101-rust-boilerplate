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
    retweets: u64,
    comments: Vec<Comment>,
}

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct Comment {
    username: String,
    content: String,
    created_at: u64,
}

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct UserProfile {
    username: Username,
    password: String, 
    profile_picture_url: Option<String>,
    bio: Option<String>,
}

impl Storable for Tweet {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl Storable for Comment {
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

impl Storable for UserProfile {
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

impl BoundedStorable for Comment {
    const MAX_SIZE: u32 = 512;
    const IS_FIXED_SIZE: bool = false;
}

impl BoundedStorable for IsUsed {
    const MAX_SIZE: u32 = 8;
    const IS_FIXED_SIZE: bool = false;
}

impl BoundedStorable for UserPrincipal {
    const MAX_SIZE: u32 = 63; 
    const IS_FIXED_SIZE: bool = false;
}

impl BoundedStorable for Username {
    const MAX_SIZE: u32 = 64;
    const IS_FIXED_SIZE: bool = false;
}

impl BoundedStorable for UserProfile {
    const MAX_SIZE: u32 = 256; 
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

    static USER_PROFILES: RefCell<StableBTreeMap<UserPrincipal, UserProfile, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(4)))
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
    USERNAME
        .with(|usernames| usernames.borrow_mut().insert(UserPrincipal(caller()), Username(username.clone())));
    Some(())
}

#[ic_cdk::update]
fn create_tweet(payload: TweetPayload) -> Option<Tweet> {
    assert!(payload.content.len() > 0, "Content can't be empty");
    let username = USERNAME.with(|u| u.borrow().get(&UserPrincipal(caller()))).expect("Only registered users can tweet");

    let id = ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("Cannot increment id counter");

    let tweet = Tweet {
        id,
        username: username.0.clone(),
        content: payload.content,
        created_at: time(),
        likes: 0,
        retweets: 0,
        comments: Vec::new(),
    };

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
                return Err(Error::Unauthorized {
                    msg: "You are not authorized to delete this tweet".to_string(),
                });
            }

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

fn do_insert_tweet(tweet: &Tweet) {
    TWEET_STORAGE
        .with(|tweets| tweets.borrow_mut().insert(tweet.id, tweet.clone()));
}

fn do_update_tweet(tweet: &Tweet) {
    TWEET_STORAGE
        .with(|tweets| tweets.borrow_mut().insert(tweet.id, tweet.clone()));
}

fn do_delete_tweet(id: &u64) {
    TWEET_STORAGE.with(|tweets| tweets.borrow_mut().remove(id));
}

#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
    Unauthorized { msg: String },
}

fn _get_tweet(id: &u64) -> Option<Tweet> {
    TWEET_STORAGE.with(|tweets| tweets.borrow().get(id).map(|tweet| tweet.clone()))
}
#[ic_cdk::update]
fn update_profile(profile: UserProfile) -> Option<()> {
    let caller_principal = UserPrincipal(caller());
    assert!(USERNAME
        .with(|usernames| usernames.borrow().get(&caller_principal))
        .is_some(), "User not registered.");

    USER_PROFILES.with(|profiles| {
        profiles.borrow_mut().insert(caller_principal.clone(), profile.clone())
    });

    Some(())
}

#[ic_cdk::query]
fn get_profile(username: String) -> Option<UserProfile> {
    let principal_result = Principal::from_text(username.clone());
    let principal = match principal_result {
        Ok(principal) => principal,
        Err(err) => {
            println!("Error creating principal: {:?}", err);
            return None;
        }
    };
    let principal = UserPrincipal(principal);
        assert!(USERNAME
        .with(|usernames| usernames.borrow().get(&principal))
        .is_some(), "User not found.");

    USER_PROFILES
    .with(|profiles| profiles.borrow().get(&principal))
    .map(|profile| profile.clone())
}

ic_cdk::export_candid!();
