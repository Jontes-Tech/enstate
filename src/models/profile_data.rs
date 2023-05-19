use ethers::{providers::{Middleware, ProviderError}, types::H160};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ProfileData {
    pub name: String,
    pub address: Option<String>,
    pub avatar: Option<String>,
}

pub enum ProfileDataError {
    NotFound,
}

impl ProfileData {
    pub async fn from_address(address: H160, state: &AppState) -> Result<Self, ProfileDataError> {
        let cache_key = format!("a:{}", address);

        let mut redis = state.redis.clone();

        // Get value from the cache otherwise compute
        let name: String = if let Ok(cached_value) = redis.get(&cache_key).await {
            cached_value
        } else {
            let vx = address;
            let v = state.fallback_provider.lookup_address(vx);
    
            let result = v.await;
    
            let result = match result {
                Ok(result) => result,
                Err(error) => match error {
                    ProviderError::EnsError(_error) => {
                        println!("ENS Error resolving address: {:?}", _error);
    
                        // Cache the value
                        let _: () = redis.set(&cache_key, "").await.unwrap();
    
                        // Expire the value after 5 minutes
                        let _: () = redis.expire(&cache_key, 300).await.unwrap();
    
                        return Err(ProfileDataError::NotFound);
                    }
                    _ => {
                        println!("Error resolving address: {:?}", error);
                        return Err(ProfileDataError::NotFound);
                    }
                },
            };
    
            // Cache the value
            let _: () = redis.set(&cache_key, &result).await.unwrap();
    
            // Expire the value after 5 minutes
            let _: () = redis.expire(&cache_key, 300).await.unwrap();
    
            result
        };    

        Self::from_name(&name, state).await
    }

    pub async fn from_name(name: &str, state: &AppState) -> Result<Self, ProfileDataError> {
        let cache_key = format!("n:{}", name);

        let mut redis = state.redis.clone();

        // Get value from the cache otherwise compute
        if let Ok(value) = redis.get(&cache_key).await as Result<String, _> {
            if value.len() > 0 {
                let entry: Self = serde_json::from_str(value.as_str()).unwrap();

                return Ok(entry);
            }

            return Err(ProfileDataError::NotFound);
        }

        // Get the address from the name
        let address_request = state.provider.resolve_name(name);

        let address = match address_request.await {
            Ok(result) => result,
            Err(e) => {
                println!("Error resolving name: {:?}", e);
                return Err(ProfileDataError::NotFound);
            }
        };

        // Get the avatar from the name
        let avatar_request = state.provider.resolve_avatar(name);

        let avatar = match avatar_request.await.ok() {
            Some(result) => Some(result.to_string()),
            None => None,
        };

        // Create the NameResponse
        let value = Self {
            name: name.to_string(),
            address: Some(format!("{:?}", address)),
            avatar,
        };

        let response = serde_json::to_string(&value).unwrap();

        // Cache the value
        let _: () = redis.set(&cache_key, &response).await.unwrap();

        // Expire the value after 5 minutes
        let _: () = redis.expire(&cache_key, 300).await.unwrap();

        // Return `value` as json string
        Ok(value)
    }
}