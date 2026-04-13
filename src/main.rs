use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Display, Formatter};
use std::io;
use chrono::Utc;
use once_cell::sync::Lazy;
use tokio::net::TcpStream;
use sqlx::mysql::MySqlPoolOptions;
use sqlx::{MySqlPool};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::try_join;
use crate::auth_jwt::Claim;
use crate::dao::{ArticleDAO, CommentDAO, UserDAO};
use crate::entity::{Article, Comment, User};
use crate::Request::{DELETE, GET, PATCH, PatchPwd, POST, LOGIN, REFRESH, REGISTER};

pub static ALLOWED_FIELDS:Lazy<HashMap<&'static str,HashSet<&'static str>>> = Lazy::new(||{
    let mut map = HashMap::new();
    map.insert("USER",HashSet::from(["id","name","age","created_at","password","Auth"]));
    map.insert("ARTICLE",HashSet::from(["id","author_id","created_at","content","Auth"]));
    map.insert("COMMENT",HashSet::from(["id","content","author_id","article_id","created_at","Auth"]));
    map
});
pub static NEC_FIELDS_AUTH:Lazy<HashMap<&'static str,HashSet<&'static str>>> = Lazy::new(||{
    let mut map = HashMap::new();
    map.insert("USER",HashSet::from(["name","password"]));
    map.insert("ARTICLE",HashSet::from(["content"]));
    map.insert("COMMENT",HashSet::from(["content","article_id"]));
    map
});
pub static NEC_FIELDS:Lazy<HashMap<&'static str,HashSet<&'static str>>> = Lazy::new(||{
    let mut map =HashMap::new();
    map.insert("USER",HashSet::from(["name","password","id"]));
    map.insert("ARTICLE",HashSet::from(["content","author_id"]));
    map.insert("COMMENT",HashSet::from(["content","author_id","article_id"]));
    map
});
pub fn get_nec_fields() ->&'static Lazy<HashMap<&'static str,HashSet<&'static str>>>{
    if USE_AUTH{
           &NEC_FIELDS_AUTH
    }else{
        &NEC_FIELDS
    }
}
pub static USE_AUTH:bool = true;
pub static ADMIN_USER_ID:u32 = 0u32;
pub mod auth_jwt{
    use chrono::Duration;
    use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
    use rand::Rng;
    use serde::{Serialize, Deserialize};

    static SECRET: &[u8] = b"randomrandomrandomsysueducnhu35at";
    #[derive(Serialize, Deserialize, Copy, Clone, Debug)]
    pub struct Claim{
        pub user_id: u32,
        pub exp: usize,
        pub token_version: u32
    }

    impl Claim{
        pub fn create_token(user_id:u32)->(String,String){
            let exp = chrono::Utc::now() + Duration::days(7);
            let claim = Claim{
                user_id,
                exp: exp.timestamp() as usize,
                token_version: 1
            };
            let a = encode(
                &Header::default(),
                &claim,
                &EncodingKey::from_secret(SECRET),
            ).expect("create token error");
            let b = rand::thread_rng()
                .sample_iter(&rand::distributions::Alphanumeric)
                .take(64)
                .map(char::from)
                .collect();
            (a,b)
        }
        pub fn verify_token(token:&str)->Result<Self,jsonwebtoken::errors::Error>{
            let validation = Validation::new(Algorithm::HS256);
            let data = decode::<Claim>(&token, &DecodingKey::from_secret(SECRET), &validation)?;
            Ok(data.claims)
        }
    }
}
pub mod entity{
    #[derive(Debug,sqlx::FromRow,serde::Serialize)]
    pub struct User{
        pub id:u32,
        pub name:String,
        #[serde(skip)]
        pub password: String,
        #[serde(skip)]
        pub token_version: u32,
        pub age:Option<u32>,
        pub created_at:Option<chrono::NaiveDate>
    }

    #[derive(Debug,sqlx::FromRow,serde::Serialize)]
    pub struct Article{
        pub id:u32,
        pub content:String,
        pub author_id:u32,
        pub created_at:Option<chrono::NaiveDate>
    }
    #[derive(Debug,sqlx::FromRow,serde::Serialize)]
    pub struct Comment{
        pub id:u32,
        pub content:String,
        pub author_id: u32,
        pub article_id: u32,
        pub created_at:Option<chrono::NaiveDate>
    }
    #[derive(serde::Serialize)]
    pub struct ArticleView{
        pub id:u32,
        pub content:String,
        pub author: User,
        pub comments: Vec<Comment>,
        pub created_at:Option<chrono::NaiveDate>
    }
    #[derive(serde::Serialize)]
    pub struct CommentView{
        pub id:u32,
        pub content:String,
        pub author: User,
        pub created_at:Option<chrono::NaiveDate>
    }
}
impl Display for User{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}",serde_json::to_string(&self).expect("error turning user to json"))
    }
}
impl Display for Article{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}",serde_json::to_string(&self).expect("error turning article to json"))
    }
}
impl Display for Comment{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}",serde_json::to_string(&self).expect("error turning comment to json"))
    }
}
pub mod dao{
    use std::collections::HashMap;
    use chrono::{ Duration, Local};
    use sqlx::{MySqlPool, Row};
    use crate::entity::*;
    pub struct UserDAO{}
    impl UserDAO{
        pub async fn get_by_id(pool:&MySqlPool, id:u32) ->anyhow::Result<User>{
            let user = sqlx::query_as!(User,"select id,name,age,created_at,token_version,password from user where id = ?",id).fetch_one(pool).await?;
            Ok(user)
        }
        pub async fn create(pool:&MySqlPool,name:&str,age:u32,encoded_password:&str)->anyhow::Result<User>{
            let result = sqlx::query!("insert into user(name,age,created_at,password,token_version)values(?,?,?,?,?)",name,age,chrono::Utc::now().date_naive(),encoded_password,1).execute(pool).await?;
            Ok(Self::get_by_id(pool, result.last_insert_id() as u32).await?)
        }
        pub async fn get(pool:&MySqlPool,map:&HashMap<String,String>)->anyhow::Result<Vec<User>>{
            let mut conditions:Vec<String> = map.iter().filter(|(a,_)|{a.as_str()!="Auth"}).map(|(a,_)|{format!("{} = ?",a)}).collect();
            conditions.push("1 = 1".to_string());
            let where_clause = conditions.join(" AND ");
            let sql = format!("select * from user where {}",where_clause);
            eprintln!("{}",sql);
            let mut query = sqlx::query_as::<_,User>(&sql);
            for (_,value) in map.iter(){
                query = query.bind(value.as_str());
            }
            let rows = query.fetch_all(pool).await?;
            Ok(rows)
        }
        pub async fn patch(pool:&MySqlPool,map:&HashMap<String,String>,id:&str)->anyhow::Result<User>{
            let primary_key:u32 = id.parse()?;
            let update_msg:Vec<String> = map.iter().filter(|(a,_)| {a.as_str()!="id"}).map(|(key,_value)|{format!("{} = ?",key)}).collect();
            let update_str = update_msg.join(",");
            let sql = format!("update user set {} where id = {}",update_str,primary_key);
            let mut query = sqlx::query(&sql);
            for (_,v) in map.iter(){
                query = query.bind(v.as_str());
            }
            let _rows = query.execute(pool).await?;
            Self::get_by_id(pool,primary_key).await
        }
        pub async fn delete(pool:&MySqlPool,map:&HashMap<String,String>)->anyhow::Result<()>{
            let mut conditions:Vec<String> = map.iter().filter(|(a,_)|{a.as_str()!="Auth"}).map(|(a,_)|{format!("{} = ?",a)}).collect();
            let where_clause = conditions.join(" AND ");
            let sql = format!("delete from user where {}",where_clause);
            eprintln!("{}",sql);
            let mut query = sqlx::query(&sql);
            for(_,v) in map.iter(){
                query = query.bind(v.as_str());
            }
            let _rows = query.execute(pool).await?;
            Ok(())
        }
        pub async fn update_pwd(pool:&MySqlPool,new_encoded_password:&str,user_id:u32)->anyhow::Result<()>{
            sqlx::query!("update user set password = ? where id = ?",new_encoded_password,user_id).execute(pool).await?;
            Self::delete_refresh_token(pool,user_id).await?;
            Ok(())
        }
        pub async fn create_refresh_token(pool:&MySqlPool,refresh_token:&str,user_id:u32)->anyhow::Result<()>{
            let sql = "insert into user_refresh_token(refresh_token,user_id,expire_time)values(?,?,?)";
            let mut query = sqlx::query(&sql);
            query = query.bind(refresh_token);
            query = query.bind(user_id);
            query = query.bind(Local::now()+Duration::days(7));
            query.execute(pool).await?;
            Ok(())
        }
        pub async fn delete_refresh_token(pool:&MySqlPool,user_id:u32)->anyhow::Result<()>{
            let sql = "delete from user_refresh_token where user_id = ?";
            let mut query = sqlx::query(&sql);
            query = query.bind(user_id);
            query.execute(pool).await?;
            Ok(())
        }
        pub async fn refresh_token(pool:&MySqlPool,refresh_token:&str)->anyhow::Result<u32>{
            let sql = "select user_id from user_refresh_token where refresh_token = ? and expire_time > NOW()";
            let mut query = sqlx::query(&sql);
            query = query.bind(refresh_token);
            let res:u32 = query.fetch_one(pool).await?.get("user_id");
            Ok(res)
        }
    }
    pub struct ArticleDAO{}
    impl ArticleDAO{
        pub async fn get_by_id(pool:&MySqlPool, id:u32) ->anyhow::Result<Article>{
            let article = sqlx::query_as!(Article,"select id,content,author_id,created_at from article where id = ?",id).fetch_one(pool).await?;
            Ok(article)
        }
        pub async fn create(pool:&MySqlPool,content:&str,author_id:u32)->anyhow::Result<Article>{
            let result = sqlx::query!("insert into article(content,author_id,created_at)values(?,?,?)",content,author_id,chrono::Utc::now().date_naive()).execute(pool).await?;
            Ok(Self::get_by_id(pool,result.last_insert_id()as u32).await?)
        }
        pub async fn get(pool:&MySqlPool,map:&HashMap<String,String>)->anyhow::Result<Vec<Article>>{
            let mut conditions:Vec<String> = map.iter().filter(|(a,_)| {a.as_str()!="Auth"}).map(|(a,_)|{format!("{} = ?",a)}).collect();
            conditions.push("1 = 1".to_string());
            let where_clause = conditions.join(" AND ");
            let sql = format!("select * from article where {}",where_clause);
            let mut query = sqlx::query_as::<_,Article>(&sql);
            for (_,value) in map.iter(){
                query = query.bind(value.as_str());
            }
            let rows = query.fetch_all(pool).await?;
            Ok(rows)
        }
        pub async fn patch(pool:&MySqlPool,map:&HashMap<String,String>)->anyhow::Result<Article>{
            let primary_key:u32 = map.get("id").unwrap().parse()?;
            let update_msg:Vec<String> = map.iter().filter(|(a,_)| {a.as_str()!="id"&&a.as_str()!="Auth"}).map(|(key,_value)|{format!("{} = ?",key)}).collect();
            let update_str = update_msg.join(",");
            let sql = format!("update article set {} where id = {}",update_str,primary_key);
            let mut query = sqlx::query(&sql);
            for (_,v) in map.iter(){
                query = query.bind(v.as_str());
            }
            let _rows = query.execute(pool).await?;
            Self::get_by_id(pool,primary_key).await
        }
        pub async fn delete(pool:&MySqlPool,map:&HashMap<String,String>)->anyhow::Result<()>{
            let mut conditions:Vec<String> = map.iter().filter(|(s,_)|{s.as_str()!="Auth"}).map(|(a,_)|{format!("{} = ?",a)}).collect();
            conditions.push("1 = 1".to_string());
            let where_clause = conditions.join(" AND ");
            let sql = format!("delete from article where {}",where_clause);
            let mut query = sqlx::query(&sql);
            for(_,v) in map.iter(){
                query = query.bind(v.as_str());
            }
            let _rows = query.execute(pool).await?;
            Ok(())
        }
    }
    pub struct CommentDAO{}
    impl CommentDAO{
        pub async fn get_by_id(pool:&MySqlPool,id:u32)->anyhow::Result<Comment>{
            let comment = sqlx::query_as!(Comment,"select * from comment where id = ?",id).fetch_one(pool).await?;
            Ok(comment)
        }
        pub async fn create(pool:&MySqlPool,content:&str,author_id:u32,article_id:u32)->anyhow::Result<Comment>{
            let result = sqlx::query!("insert into comment(content,author_id,article_id,created_at)values(?,?,?,?)",content,author_id,article_id,chrono::Utc::now().date_naive()).execute(pool).await?;
            Ok(Self::get_by_id(pool,result.last_insert_id() as u32).await?)
        }
        pub async fn get(pool:&MySqlPool,map:&HashMap<String,String>)->anyhow::Result<Vec::<Comment>>{
            let mut conditions:Vec<String> = map.iter().filter(|(a,_)|{a.as_str()!="Auth"}).map(|(a,_)|{format!("{} = ?",a)}).collect();
            conditions.push("1 = 1".to_string());
            let where_clause = conditions.join(" AND ");
            let sql = format!("select * from comment where {}",where_clause);
            let mut query = sqlx::query_as::<_,Comment>(&sql);
            for (_,value) in map.iter(){
                query = query.bind(value.as_str());
            }
            let rows = query.fetch_all(pool).await?;
            Ok(rows)
        }
        pub async fn patch(pool:&MySqlPool,map:&HashMap<String,String>)->anyhow::Result<Comment>{
            let primary_key:u32 = map.get("id").unwrap().parse()?;
            let update_msg:Vec<String> = map.iter().filter(|(a,_)| {a.as_str()!="id"&&a.as_str()!="Auth"}).map(|(key,_value)|{format!("{} = ?",key)}).collect();
            let update_str = update_msg.join(",");
            let sql = format!("update comment set {} where id = {}",update_str,primary_key);
            let mut query = sqlx::query(&sql);
            for (_,v) in map.iter(){
                query = query.bind(v.as_str());
            }
            let _rows = query.execute(pool).await?;
            Self::get_by_id(pool,primary_key).await
        }
        pub async fn delete(pool:&MySqlPool,map:&HashMap<String,String>)->anyhow::Result<()>{
            let mut conditions:Vec<String> = map.iter().filter(|(a,_)|{a.as_str()!="Auth"}).map(|(a,_)|{format!("{} = ?",a)}).collect();
            conditions.push("1 = 1".to_string());
            let where_clause = conditions.join(" AND ");
            let sql = format!("delete from comment where {}",where_clause);
            let mut query = sqlx::query(&sql);
            for (_,v) in map.iter(){
                query = query.bind(v.as_str());
            }
            let _rows = query.execute(pool).await?;
            Ok(())
        }
    }
}
#[derive(Debug)]
struct ParseError<'a>{
    msg: &'a str
}
impl<'a> Display for ParseError<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f,"Parse Error: {}\r\n",self.msg)
    }
}
impl std::error::Error for ParseError<'_>{}
enum Request{
    GET(String,HashMap<String,String>),
    POST(String,HashMap<String,String>),
    PATCH(String,HashMap<String,String>),
    DELETE(String,HashMap<String,String>),
    PatchPwd(HashMap<String,String>),
    LOGIN(String,String),
    REFRESH(String),
    REGISTER(HashMap<String,String>)
}
impl Request{
    fn parse(line: &str) ->Result<Self, ParseError<'_>> {
        let list: Vec<String> = line.split("\n").map(|s|{s.trim().to_string()}).collect();
        if list.len() < 1{
            return Err(ParseError{msg:"the request should at least contain method and table name!"})
        }
        let method_and_field:Vec<&str> = list[0].split(" ").map(|a|{a.trim()}).collect();
        if method_and_field.len() != 2{return Err(ParseError{msg:"invalid format"});}
        let s = method_and_field[0];
        let f = method_and_field[1];
        if f != "USER" && f != "ARTICLE" && f != "COMMENT"{return Err(ParseError{msg:"not such entity"})}
        let mut map = HashMap::new();
        for i in list[1..].into_iter(){
            let key_value:Vec<String> = i.split("=").map(|a|{a.trim()}).map(String::from).collect();
            if key_value.len()!=2{return Err(ParseError{msg:"the request should be in a key=value format"});}
            eprintln!("RECEIVED ARG: key: {:?},value: {:?}",key_value[0],key_value[1]);
            if (s == "GET" || s == "POST" || s == "PATCH")&&!ALLOWED_FIELDS.get(f).unwrap().contains(key_value[0].as_str())
            {return Err(ParseError{msg:"invalid argument"});}
            map.insert(key_value[0].trim().to_string(),key_value[1].trim().to_string());
        }
        match s {
            "GET" => {
                Ok(GET(f.to_string(), map))
            },
            "POST" =>{
                for &i in get_nec_fields().get(f).unwrap().into_iter(){
                    if !map.contains_key(i){
                        return Err(ParseError{msg:"not enough argument"});
                    }
                }
                Ok(POST(f.to_string(), map))
            },
            "PATCH" =>{
                if (f == "ARTICLE" || f == "COMMENT") &&!map.contains_key("id"){
                    return Err(ParseError{msg:"no primary key found"});
                }
                if f == "ARTICLE" && map.contains_key("author_id"){
                    return Err(ParseError{msg:"author_id is unchangeable"});
                }
                if f == "COMMENT" && map.contains_key("author_id") || map.contains_key("article_id"){
                    return Err(ParseError{msg:"author_id or article_id is unchangeable"});
                }
                Ok(PATCH(f.to_string(), map))
            }
            "DELETE" =>{
                Ok(DELETE(f.to_string(), map))
            },
            "PATCH_PWD" =>{
                if !map.contains_key("id") || !map.contains_key("new_password")||!map.contains_key("old_password"){
                    return Err(ParseError{msg:"update password should include both old and new password and user id"});
                }
                Ok(PatchPwd(map))
            },
            "LOGIN" =>{
                if !map.contains_key("name") || !map.contains_key("password"){
                    return Err(ParseError{msg:"name and password are required for login"});
                }
                Ok(LOGIN(map.get("name").unwrap().to_string(),map.get("password").unwrap().to_string()))
            },
            "REFRESH" =>{
                if !map.contains_key("refresh_token"){
                    return Err(ParseError{msg:"refresh_token are required"});
                }
                Ok(REFRESH(map.get("refresh_token").unwrap().to_string()))
            },
            "REGISTER" =>{
                if f != "USER"{
                    return Err(ParseError{msg:"use REGISTER USER"});
                }
                for &i in get_nec_fields().get("USER").unwrap().into_iter(){
                    if !map.contains_key(i){
                        return Err(ParseError{msg:"not enough argument"});
                    }
                }
                Ok(REGISTER(map))
            }
            _val =>{
                Err(ParseError{msg:"No such method!"})
            }
        }
    }
}
enum Response{
    NotFound,
    Created(String),
    OK(String),
    BadRequest,
    NoAuth
}
enum AuthResult{
    TokenExpired,
    TokenVersionExpired,
    InvalidToken,
    NoAuth,
    Ok(Claim)
}
impl AuthResult {
    fn to_result(self) -> Result<(), AuthResult> {
        match self {
            AuthResult::Ok(_) => Ok(()),
            err => Err(err),
        }
    }
}
impl Display for Response{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            Response::NotFound => { "Status: NOT_FOUND;\r\n".to_string() }
            Response::Created(val) => { format!("Status: CREATED\r\n {};\r\n", val) }
            Response::OK(val) => { format!("Status: OK\r\n {};\r\n", val) }
            Response::BadRequest => { "Status: BAD_REQUEST;\r\n".to_string() }
            Response::NoAuth=>{"Status: NoAuth;\r\n".to_string()}
        };
        write!(f, "{}", str)
    }
}
// have auth?
// TOKEN SIDE: parse_token_error => invalid_token, token_expired => token_expired  :FAST OPERATION
// USER SIDE: !user exist || !user token version correct => no_auth :DATABASE_OP: in user
// ENTITY SIDE: !user is auth => no_auth :DATABASE_OP: in article or in comment
// the user side and entity side are independent so can be done async
fn check_token_status(map:&HashMap<String,String>)->AuthResult{
    if !map.contains_key("Auth"){
        return AuthResult::NoAuth
    }
    let token = map.get("Auth").unwrap();
    if let Ok(claim) = Claim::verify_token(token){
        if claim.user_id == ADMIN_USER_ID{
            return AuthResult::Ok(claim);
        }
        if claim.exp < Utc::now().timestamp() as usize{
            AuthResult::TokenExpired
        }else {
            AuthResult::Ok(claim)
        }
    }else{
        AuthResult::InvalidToken
    }
}
async fn user_auth_check(pool:&MySqlPool,claim:Claim)->AuthResult{
    eprintln!("checking claim: {:?}",claim);
    let user_res = UserDAO::get_by_id(pool, claim.user_id).await;
    if let Ok(user) = user_res {
        if user.token_version > claim.token_version {
            return AuthResult::TokenVersionExpired;
        }
        AuthResult::Ok(claim)
    } else {
       AuthResult::NoAuth
    }
}

async fn entity_auth_check(pool:&MySqlPool,claim: Claim,(entity,id): (&str,u32))->AuthResult{
    match entity {
        "ARTICLE" =>{
            if let Ok(article) = ArticleDAO::get_by_id(pool,id).await{
                if article.author_id == claim.user_id{
                    AuthResult::Ok(claim)
                }else{
                    AuthResult::NoAuth
                }
            }else{
                AuthResult::NoAuth
            }
        },
        "COMMENT" =>{
            if let Ok(comment) = CommentDAO::get_by_id(pool,id).await{
                if comment.author_id == claim.user_id{
                    AuthResult::Ok(claim)
                }else{
                    AuthResult::NoAuth
                }
            }else{
                AuthResult::NoAuth
            }
        },
        _ => AuthResult::NoAuth
    }
}
    // get => no auth require
    /* post =>{
        user => claim is valid and creator exists(not expired)
        article => claim is valid and creator exists(author_id is automatically set to claim's id),
        comment => claim is valid and creator exists and article related exists(author_id is automatically set to claim's id)
        }
    patch =>{
        user => claim is valid and creator exists (user_id is automatically set to claim's id unless it's from admin)
        article => claim is valid and creator exists and article's author_id == claim's user_id intention to change id,author_id is not allowed,
        comment => claim is valid and creator exists and comment's author_id == claim's user_id intention to change id,author_id,article_id is not allowed,
    },
    delete =>{
        user => claim is valid and creator exists and user_id is automatically set to claim's id unless it's from admin,
        article => claim is valid and creator exists and article exists and (claim's id == author_id unless it's from admin)
        comment => claim is valid and creator exists and comment exists and (claim's id == author_id unless it's from admin)
    },
    patch_pwd =>{
        user => claim is valid and creator exists and user_id is automatically set to claim's id,
    }
    claim is valid <====> fn decode()
    (except get request)
    creator exists and admin case{
        user && patch_pwd=>{
            return true if admin_id == claim's id,
            one db search on user(exist?),
        },
        article =>{
            return true if admin_id == claim's id,
            one db search on user (exist?)
        },
        comment =>{
            return true if admin_id == claim's id
            one db search on user (exist?)
        }
    } <=====> future_1 user_future,

    (only in patch and delete request)
    claim's id == author_id && article exits && comment exists{
        user && patch_pwd => true,
        article =>{
            one db search for article by article_id and compare author_id to claim's id (exists)
        },
        comment =>{
            one db search for comment by comment_id and compare author_id to claim's id (exists)
        }
    } <=====> future_2 entity_future
*/
async fn patch_delete_auth_handle(pool:&MySqlPool,s:&String,map:&HashMap<String,String>)->AuthResult {
        match check_token_status(map) {
            AuthResult::Ok(claim) => {
                //let claim = Claim::verify_token(map.get("Auth").unwrap()).unwrap();
                if claim.user_id == ADMIN_USER_ID {
                    return AuthResult::Ok(claim);
                }
                match s.as_str() {
                    "USER" => {
                        user_auth_check(pool, claim).await
                    },
                    "ARTICLE" => {
                        let f1 = async { user_auth_check(pool, claim).await.to_result() };
                        let f2 = async { entity_auth_check(pool, claim, ("ARTICLE", map.get("id").unwrap().parse().unwrap())).await.to_result() };
                        match try_join!(f1,f2) {
                            Ok(_) => { AuthResult::Ok(claim) }
                            Err(e) => { e }
                        }
                    },
                    "COMMENT" => {
                        let f1 = async { user_auth_check(pool, claim).await.to_result() };
                        let f2 = async { entity_auth_check(pool, claim, ("COMMENT", map.get("id").unwrap().parse().unwrap())).await.to_result() };
                        match try_join!(f1,f2) {
                            Ok(_) => { AuthResult::Ok(claim) }
                            Err(e) => { e }
                        }
                    },
                    _ => AuthResult::NoAuth
                }
            },
            val => val
        }
    }
async fn have_auth(pool: &MySqlPool, request: &Request) -> AuthResult {
    if !USE_AUTH {
        AuthResult::Ok(Claim{user_id:0, exp:Utc::now().timestamp() as usize,token_version:1})
    } else {
        match request{
            GET(_s,_map)=>{
                AuthResult::Ok(Claim{user_id:0, exp:Utc::now().timestamp() as usize,token_version:1})
            },
            POST(s,map)=>{
                match check_token_status(map){
                    AuthResult::Ok(claim) => {
                        //let claim = Claim::verify_token(map.get("Auth").unwrap()).unwrap();
                        if claim.user_id == ADMIN_USER_ID{
                            return AuthResult::Ok(claim);
                        }
                        match s.as_str(){
                            val if val == "USER" || val == "ARTICLE" || val == "COMMENT"=>{
                              user_auth_check(pool,claim).await
                            },
                            _ => AuthResult::NoAuth
                        }
                    },
                    val => val
                }
            },
            PATCH(s,map)=>{
                patch_delete_auth_handle(pool,s,map).await
            },
            DELETE(s,map)=>{
                patch_delete_auth_handle(pool,s,map).await
            },
            PatchPwd(map)=>{
                match check_token_status(map){
                    AuthResult::Ok(claim)=>{
                        //let claim = Claim::verify_token(map.get("Auth").unwrap()).unwrap();
                            if claim.user_id == ADMIN_USER_ID{
                                return AuthResult::Ok(claim)
                            }
                            user_auth_check(pool,claim).await
                    },
                    val=>val
                }
            },
            LOGIN(_name,_password)=>{
                AuthResult::Ok(Claim{user_id:0, exp:Utc::now().timestamp() as usize,token_version:1})
            },
            REFRESH(_refresh_token)=>{
                AuthResult::Ok(Claim{user_id:0, exp:Utc::now().timestamp() as usize,token_version:1})
            },
            Request::REGISTER(_map)=>{
                AuthResult::Ok(Claim{user_id:0, exp:Utc::now().timestamp() as usize,token_version:1})
            }
        }
    }
}
async fn process_request(pool:&MySqlPool,request: &Request)->Response{
    if let AuthResult::Ok(claim) = have_auth(pool,request).await{
        match request{
            GET(field, map) => {
                match field.as_str(){
                    "USER"=>{
                        match UserDAO::get(pool,map).await{
                            Err(_e) => Response::BadRequest,
                            Ok(vec) =>{
                                if vec.is_empty(){
                                    Response::NotFound
                                }else {
                                    Response::OK(serde_json::to_string(&vec).unwrap())
                                }
                            }
                        }
                    },
                    "ARTICLE"=>{
                        match ArticleDAO::get(pool,map).await{
                            Err(_e) => Response::BadRequest,
                            Ok(vec) =>{
                                Response::OK(serde_json::to_string(&vec).unwrap())
                            }
                        }
                    },
                    "COMMENT"=>{
                        match CommentDAO::get(pool,map).await{
                            Err(_e) => Response::BadRequest,
                            Ok(vec) =>{
                                if vec.is_empty(){
                                    Response::NotFound
                                }else {
                                    Response::OK(serde_json::to_string(&vec).unwrap())
                                }
                            }
                        }
                    },
                    _val =>{
                        Response::BadRequest
                    }
                }
            }
            POST(field, map) => {
                match field.as_str(){
                    "USER" =>{
                        let encoded_password = bcrypt::hash(map.get("password").unwrap(),10).unwrap();
                        match UserDAO::create(pool, map.get("name").unwrap(), map.get("age").unwrap().parse::<u32>().unwrap(),encoded_password.as_str()).await {
                            Err(_e) => Response::BadRequest,
                            Ok(item) => Response::Created(item.to_string())
                        }
                    },
                    "ARTICLE"=>{
                        let author_id = if USE_AUTH {claim.user_id}else{map.get("author_id").unwrap().parse::<u32>().unwrap()};
                        match ArticleDAO::create(pool, map.get("content").unwrap(), author_id).await {
                            Err(_) => Response::BadRequest,
                            Ok(item) => Response::Created(item.to_string())
                        }
                    },
                    "COMMENT"=>{
                        let author_id = if USE_AUTH {claim.user_id}else{map.get("author_id").unwrap().parse::<u32>().unwrap()};
                        match CommentDAO::create(pool, map.get("content").unwrap(), author_id, map.get("article_id").unwrap().parse().unwrap()).await {
                            Err(_) => Response::BadRequest,
                            Ok(item) => Response::Created(item.to_string())
                        }
                    },
                    _ =>{Response::BadRequest}
                }
            }
            PATCH(field, map) => {
                match field.as_str() {
                    "USER"=>{
                        let id = if USE_AUTH{
                            claim.user_id.to_string()
                        }else{
                            map.get("id").unwrap().to_string()
                        };
                        match UserDAO::patch(pool,map,id.as_str()).await{
                            Ok(item) => {Response::OK(item.to_string())}
                            Err(_) => {Response::BadRequest}
                        }
                    },
                    "ARTICLE"=>{
                        match ArticleDAO::patch(pool,map).await{
                            Ok(item) => {Response::OK(item.to_string())}
                            Err(_) => {Response::BadRequest}
                        }
                    },
                    "COMMENT"=>{
                        match CommentDAO::patch(pool,map).await{
                            Ok(item) => {Response::OK(item.to_string())}
                            Err(_) => {Response::BadRequest}
                        }
                    }
                    _=>Response::BadRequest,
                }
            }
            DELETE(field, map) => {
                match field.as_str() {
                    "USER"=>{
                        let _id = if USE_AUTH{
                            claim.user_id.to_string()
                        }else{
                            map.get("id").unwrap().to_string()
                        };
                        match UserDAO::delete(pool,map).await{
                            Ok(_item) => {Response::OK("deleted".to_string())}
                            Err(_) => {Response::BadRequest}
                        }
                    },
                    "ARTICLE"=>{
                        //entity_auth_check(pool,claim,("article",claim))
                        match ArticleDAO::delete(pool,map).await{
                            Ok(_item) => {Response::OK("deleted".to_string())}
                            Err(_) => {Response::BadRequest}
                        }
                    },
                    "COMMENT"=>{
                        match UserDAO::delete(pool,map).await{
                            Ok(_item) => {Response::OK("deleted".to_string())}
                            Err(_) => {Response::BadRequest}
                        }
                    }
                    _=>Response::BadRequest,
                }
            }
            PatchPwd(map)=>{
                let old_password_encoded = UserDAO::get_by_id(pool,map.get("id").unwrap().parse().unwrap()).await.unwrap().password;
                if !bcrypt::verify(map.get("old_password").unwrap(),old_password_encoded.as_str()).unwrap(){
                    Response::BadRequest
                }else {
                    let new_password_encoded = bcrypt::hash(map.get("new_password").unwrap(),10).unwrap();
                    UserDAO::update_pwd(pool, new_password_encoded.as_str(), map.get("id").unwrap().parse().unwrap()).await.expect("IO error");
                    Response::OK("Password Updated".to_string())
                }
            }
            LOGIN(name,password)=>{
                let mut hash = HashMap::new();
                hash.insert("name".to_string(),name.to_string());
                if let Ok(users) = UserDAO::get(pool,&hash).await{
                    eprintln!("User Got");
                    if users.len()!=1{
                        Response::NotFound
                    }else{
                        let user = &users[0];
                        if bcrypt::verify(password,user.password.as_str()).unwrap(){
                            let (token,refresh_token) = Claim::create_token(user.id);
                            UserDAO::create_refresh_token(pool,refresh_token.as_str(),user.id).await.unwrap();
                            Response::OK(format!("{{token: {},refresh_token:{}}}",token,refresh_token))
                        }else{
                            Response::BadRequest
                        }
                    }
                }else{
                    Response::BadRequest
                }
            },
            REFRESH(r_token)=>{
                if let Ok(id) = UserDAO::refresh_token(pool,r_token).await{
                    Response::OK(format!("{{token: {}}}",Claim::create_token(id).0))
                }else{
                    Response::BadRequest
                }
            },
            Request::REGISTER(map)=>{
                let encoded_password = bcrypt::hash(map.get("password").unwrap(),10).unwrap();
                match UserDAO::create(pool, map.get("name").unwrap(), map.get("age").unwrap().parse::<u32>().unwrap(),encoded_password.as_str()).await {
                    Err(_e) => Response::BadRequest,
                    Ok(item) => Response::Created(item.to_string())
                }
            }
        }
    }else{
        Response::NoAuth
    }

}
async fn print_response(socket:&mut TcpStream,response: Response)->io::Result<()>{
        socket.write_all(response.to_string().as_bytes()).await
}
async fn read_line(socket:&mut TcpStream) ->Result<Vec<u8>,io::Error>{
    let mut ans = Vec::<u8>::new();
    let mut buff = [0u8;1024];
    let mut state = false;
    'p :loop{
        let cnt = socket.read(&mut buff).await?;
        if cnt == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "closed"));
        }
        for &i in &buff[..cnt]{
            if i == ';' as u8{
                state = true;
                ans.push(i);
            }else if i == b'\x08'{
                ans.pop();
            }else if i != ';' as u8 && i != '\r' as u8{
                state = false;
                ans.push(i);
            }else if i == '\r' as u8{
                if state{
                    ans.pop();
                    break 'p;
                }else{
                    ans.push(i)
                }
            }
        }
    }
    Ok(ans)
}
#[tokio::main]
async fn main()->Result<(),Box<dyn std::error::Error>> {
    let pool = MySqlPoolOptions::new().max_connections(5).connect("mysql://root:HUhao200636@localhost:3307/rust_program").await?;
    let listener = TcpListener::bind("localhost:9000").await?;
    loop {
        let (mut socket,_) = listener.accept().await?;
        let pool_clone = pool.clone();
        tokio::spawn(async move {
            loop {
                let res = read_line(&mut socket).await;
                let res_line = match res {
                    Ok(line) => line,
                    Err(e) => {eprintln!("{}",e);return;}
                };
                println!("Message: {}",String::from_utf8_lossy(&res_line).replace("\r","\\r").replace("\n","\\n"));
                let parse_line = String::from_utf8(res_line).unwrap();
                let request_res = Request::parse(parse_line.as_str());
                if let Err(e) =request_res {
                    socket.write_all(e.to_string().as_bytes()).await.expect("IO error");
                }else if let Ok(request) = request_res{
                    let response = process_request(&pool_clone, &request).await;
                    print_response(&mut socket,response).await.expect("IO error");
                }
            }
    });
    }
}
