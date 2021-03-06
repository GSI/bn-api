use errors::{ApplicationError, BigNeonError};
use models::CreateArtistRequest;
use reqwest::Client;
use serde_json;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};
use url::percent_encoding::{utf8_percent_encode, PATH_SEGMENT_ENCODE_SET};

const SPOTIFY_URL_AUTH: &'static str = "https://accounts.spotify.com/api/token";

//Look into refCell / singleton / threadlocal (tokio runtime)
#[derive(Debug)]
pub struct Spotify {
    pub auth_token: Option<String>,
    access_token: String,
    expires_at: u64,
}

#[derive(Deserialize)]
struct AuthResponse {
    access_token: String,
    expires_in: u64,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SpotifyArtist {
    #[serde(rename = "spotify_id")]
    pub id: Option<String>,
    pub name: Option<String>,
    pub href: Option<String>,
}

impl Spotify {
    pub fn connect(auth_token: Option<String>) -> Result<Spotify, reqwest::Error> {
        match &auth_token {
            Some(token) => {
                let reqwest_client = Client::new();
                let params = [("grant_type", "client_credentials")];
                let res: AuthResponse = reqwest_client
                    .post(SPOTIFY_URL_AUTH)
                    .header("Authorization", format!("Basic {}", token))
                    .form(&params)
                    .send()?
                    .json()?;

                let start = SystemTime::now();
                let since_the_epoch = start
                    .duration_since(UNIX_EPOCH)
                    .expect("Time went backwards");
                let expires_at = since_the_epoch.as_secs() + res.expires_in;

                let spotify_instance = Spotify {
                    auth_token: Some(token.to_string()),
                    access_token: res.access_token,
                    expires_at,
                };
                Ok(spotify_instance)
            }
            None => Ok(Spotify {
                auth_token: None,
                access_token: "".to_string(),
                expires_at: 0,
            }),
        }
    }

    pub fn search(&self, q: String) -> Result<Vec<CreateArtistRequest>, BigNeonError> {
        match &self.auth_token {
            Some(_auth_token) => {
                let reqwest_client = Client::new();
                let access_token = self.access_token.clone();
                let encoded_q = utf8_percent_encode(&q, PATH_SEGMENT_ENCODE_SET);
                let url = format!(
                    "https://api.spotify.com/v1/search?q={}&type=artist&access_token={}",
                    encoded_q, access_token
                );
                let res = reqwest_client.get(&url).send()?.text()?;
                let result: Value = serde_json::from_str(&res)?;
                if result.get("error").is_some() {
                    return Err(ApplicationError::new(
                        result["error"]["message"]
                            .as_str()
                            .unwrap_or("Invalid Spotify Response")
                            .to_string(),
                    )
                    .into());
                }
                let spotify_artists = result["artists"]["items"]
                    .as_array()
                    .unwrap()
                    .into_iter()
                    .map(|item| {
                        let artist = item;
                        let image_url =
                            Spotify::get_image_from_artist(&artist["images"], Some(600), None);
                        CreateArtistRequest {
                            name: artist["name"].as_str().map(|s| s.to_string()),
                            bio: Some("".to_string()),
                            spotify_id: artist["id"].as_str().map(|s| s.to_string()),
                            image_url,
                            other_image_urls: artist["images"].as_array().map(|a| {
                                a.iter()
                                    .map(|i| i["url"].as_str().map(|s| s.to_string()))
                                    .filter(|i| i.is_some())
                                    .map(|i| i.unwrap())
                                    .collect()
                            }),
                            ..Default::default()
                        }
                    })
                    .collect();
                Ok(spotify_artists)
            }
            None => Ok(vec![]),
        }
    }

    pub fn read_artist(
        &self,
        artist_id: &str,
    ) -> Result<Option<CreateArtistRequest>, BigNeonError> {
        match &self.auth_token {
            Some(_auth_token) => {
                let reqwest_client = Client::new();

                let access_token = self.access_token.clone();
                let url = format!("https://api.spotify.com/v1/artists/{}", artist_id);

                let res = reqwest_client
                    .get(&url)
                    .header("Authorization", format!("Bearer {}", access_token))
                    .send()?
                    .text()?;

                let artist: Value = serde_json::from_str(&res)?;
                if artist.get("error").is_some() {
                    return Err(ApplicationError::new(
                        artist["error"]["message"]
                            .as_str()
                            .unwrap_or("Invalid Spotify Response")
                            .to_string(),
                    )
                    .into());
                } else {
                    let image_url =
                        Spotify::get_image_from_artist(&artist["images"], Some(600), None);
                    let thumb_image_url =
                        Spotify::get_image_from_artist(&artist["images"], None, Some(300));

                    let create_artist = CreateArtistRequest {
                        name: artist["name"].as_str().map(|s| s.to_string()),
                        bio: Some("".to_string()),
                        spotify_id: artist["id"].as_str().map(|s| s.to_string()),
                        image_url,
                        thumb_image_url,
                        other_image_urls: artist["images"].as_array().map(|a| {
                            a.iter()
                                .map(|i| i["url"].as_str().map(|s| s.to_string()))
                                .filter(|i| i.is_some())
                                .map(|i| i.unwrap())
                                .collect()
                        }),

                        ..Default::default()
                    };
                    Ok(Some(create_artist))
                }
            }
            None => Err(ApplicationError::new("No Spotify Auth Token".to_string()).into()),
        }
    }

    pub fn get_image_from_artist(
        image_array: &Value,
        min_width: Option<i64>,
        max_width: Option<i64>,
    ) -> Option<String> {
        let array = match image_array.as_array() {
            Some(u) => u,
            None => return None,
        };

        if let Some(width) = min_width {
            for i in 0..array.len() {
                let value = array.get(i);
                if value.is_none() {
                    return None;
                }
                let value = value.unwrap();
                if value["width"].as_i64().unwrap_or(0) < width {
                    continue;
                }
                return value["url"].as_str().map(|s| s.to_string());
            }
        };

        if let Some(width) = max_width {
            for i in 0..array.len() {
                let value = array.get(i);
                if value.is_none() {
                    return None;
                }
                let value = value.unwrap();
                if value["width"].as_i64().unwrap_or(99999999) > width {
                    continue;
                }
                return value["url"].as_str().map(|s| s.to_string());
            }
        }

        None
    }
}
