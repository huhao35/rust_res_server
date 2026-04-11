use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Display, Formatter};
use std::io;
use once_cell::sync::Lazy;
use tokio::net::TcpStream;
use sqlx::mysql::MySqlPoolOptions;
use sqlx::{MySqlPool};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use crate::dao::{ArticleDAO, CommentDAO, UserDAO};
use crate::entity::{Article, Comment, User};
use crate::Request::{DELETE, GET, PATCH, POST};
pub static ALLOWED_FIELDS:Lazy<HashMap<&'static str,HashSet<&'static str>>> = Lazy::new(||{
    let mut map = HashMap::new();
    map.insert("USER",HashSet::from(["id","name","age","created_at"]));
    map.insert("ARTICLE",HashSet::from(["id","author_id","created_at","content"]));
    map.insert("COMMENT",HashSet::from(["id","content","author_id","article_id","created_at"]));
    map
});
pub static NEC_FIELDS:Lazy<HashMap<&'static str,HashSet<&'static str>>> = Lazy::new(||{
    let mut map = HashMap::new();
    map.insert("USER",HashSet::from(["name"]));
    map.insert("ARTICLE",HashSet::from(["author_id","content"]));
    map.insert("COMMENT",HashSet::from(["content","author_id","article_id"]));
    map
});

pub mod entity{


    #[derive(Debug,sqlx::FromRow)]
    pub struct User{
        pub id:u32,
        pub name:String,
        pub age:Option<u32>,
        pub created_at:Option<chrono::NaiveDate>
    }

    #[derive(Debug,sqlx::FromRow)]
    pub struct Article{
        pub id:u32,
        pub content:String,
        pub author_id:u32,
        pub created_at:Option<chrono::NaiveDate>
    }
    #[derive(Debug,sqlx::FromRow)]
    pub struct Comment{
        pub id:u32,
        pub content:String,
        pub author_id: u32,
        pub article_id: u32,
        pub created_at:Option<chrono::NaiveDate>
    }
    pub struct ArticleView{
        pub id:u32,
        pub content:String,
        pub author: User,
        pub comments: Vec<Comment>,
        pub created_at:Option<chrono::NaiveDate>
    }
    pub struct CommentView{
        pub id:u32,
        pub content:String,
        pub author: User,
        pub created_at:Option<chrono::NaiveDate>
    }
}
impl Display for User{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{\"id\" : \"{}\",\"name\" :\"{}\", \"age\" :\"{:?}\", \"created_at\": \"{:?}\"}}", self.id, self.name, self.age, self.created_at)
    }
}
impl Display for Article{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{\"id\" : \"{}\",\"content\" :\"{}\", \"author_id\" :\"{}\", \"created_at\": \"{:?}\"}}", self.id, self.content, self.author_id, self.created_at)
    }
}
impl Display for Comment{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{\"id\" : \"{}\",\"content\" :\"{}\", \"author_id\" :\"{}\", \"article_id\": \"{}\", \"created_at\": \"{:?}\"}}", self.id, self.content, self.author_id,self.article_id,self.created_at)
    }
}
pub mod dao{
    use std::collections::HashMap;
    use sqlx::MySqlPool;
    use crate::entity::*;
    pub struct UserDAO{}
    impl UserDAO{
        pub async fn get_by_id(pool:&MySqlPool, id:u32) ->anyhow::Result<User>{
            let user = sqlx::query_as!(User,"select id,name,age,created_at from user where id = ?",id).fetch_one(pool).await?;
            Ok(user)
        }
        pub async fn create(pool:&MySqlPool,name:&str,age:u32)->anyhow::Result<User>{
            let result = sqlx::query!("insert into user(name,age,created_at)values(?,?,?)",name,age,chrono::Utc::now().date_naive()).execute(pool).await?;
            Ok(Self::get_by_id(pool, result.last_insert_id() as u32).await?)
        }
        pub async fn get(pool:&MySqlPool,map:&HashMap<String,String>)->anyhow::Result<Vec<User>>{
            let mut conditions:Vec<String> = map.iter().map(|(a,_)|{format!("{} = ?",a)}).collect();
            conditions.push("1 = 1".to_string());
            let where_clause = conditions.join("AND");
            let sql = format!("select * from user where {}",where_clause);
            let mut query = sqlx::query_as::<_,User>(&sql);
            for (_,value) in map.iter(){
                query = query.bind(value.as_str());
            }
            let rows = query.fetch_all(pool).await?;
            Ok(rows)
        }
        pub async fn patch(pool:&MySqlPool,map:&HashMap<String,String>)->anyhow::Result<User>{
            let primary_key:u32 = map.get("id").unwrap().parse()?;
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
            let mut conditions:Vec<String> = map.iter().filter(|(a,_)| {a.as_str()!="id"}).map(|(a,_)|{format!("{} = ?",a)}).collect();
            conditions.push("1 = 1".to_string());
            let where_clause = conditions.join("AND");
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
            let update_msg:Vec<String> = map.iter().filter(|(a,_)| {a.as_str()!="id"}).map(|(key,_value)|{format!("{} = ?",key)}).collect();
            let update_str = update_msg.join(",");
            let sql = format!("update article set {} where id = {}",update_str,primary_key);
            let mut query = sqlx::query(&sql);
            for (_,v) in map.iter(){
                query = query.bind(v.as_str());
            }
            let _rows = query.execute(pool).await?;
            Self::get_by_id(pool,primary_key).await
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
            let mut conditions:Vec<String> = map.iter().map(|(a,_)|{format!("{} = ?",a)}).collect();
            conditions.push("1 = 1".to_string());
            let where_clause = conditions.join("AND");
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
            let update_msg:Vec<String> = map.iter().filter(|(a,_)| {a.as_str()!="id"}).map(|(key,_value)|{format!("{} = ?",key)}).collect();
            let update_str = update_msg.join(",");
            let sql = format!("update comment set {} where id = {}",update_str,primary_key);
            let mut query = sqlx::query(&sql);
            for (_,v) in map.iter(){
                query = query.bind(v.as_str());
            }
            let _rows = query.execute(pool).await?;
            Self::get_by_id(pool,primary_key).await
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
            if !ALLOWED_FIELDS.get(f).unwrap().contains(key_value[0].as_str()){return Err(ParseError{msg:"invalid argument"});}
            map.insert(key_value[0].trim().to_string(),key_value[1].trim().to_string());
        }
        match s {
            "GET" => {
                Ok(GET(f.to_string(), map))
            },
            "POST" =>{
                for &i in NEC_FIELDS.get(f).unwrap().into_iter(){
                    if !map.contains_key(i){
                        return Err(ParseError{msg:"not enough argument"});
                    }
                }
                Ok(POST(f.to_string(), map))
            },
            "PATCH" =>{
                if !map.contains_key("id"){
                    return Err(ParseError{msg:"no primary key found"});
                }
                Ok(PATCH(f.to_string(), map))
            }
            "DELETE" =>{
                Ok(DELETE(f.to_string(), map))
            },
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
    BadRequest
}
impl Display for Response{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            Response::NotFound => { "Status: NOT_FOUND;\r\n".to_string() }
            Response::Created(val) => { format!("Status: CREATED\r\n {};\r\n", val) }
            Response::OK(val) => { format!("Status: OK\r\n {};\r\n", val) }
            Response::BadRequest => { "Status: BAD_REQUEST;\r\n".to_string() }
        };
        write!(f, "{}", str)
    }
}
async fn process_request(pool:&MySqlPool,request: &Request)->Response{
    //let s = request.get_field();
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
                                Response::OK(vec.into_iter().map(|u| { u.to_string() }).collect())
                            }
                        }
                    }
                },
                "ARTICLE"=>{
                    match ArticleDAO::get(pool,map).await{
                        Err(_e) => Response::BadRequest,
                        Ok(vec) =>{
                            if vec.is_empty(){
                                Response::NotFound
                            }else {
                                Response::OK(vec.into_iter().map(|u| { u.to_string() }).collect())
                            }
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
                                Response::OK(vec.into_iter().map(|u| { u.to_string() }).collect())
                            }
                        }
                    }
                },
                _val=>{
                    Response::BadRequest
                }
            }
        }
        POST(field, map) => {
            match field.as_str(){
                "USER" =>{
                    match UserDAO::create(pool, map.get("name").unwrap(), map.get("age").unwrap().parse::<u32>().unwrap()).await {
                        Err(_e)=>Response::BadRequest,
                        Ok(item)=>Response::Created(item.to_string())
                    }
                },
                "ARTICLE"=>{
                    match ArticleDAO::create(pool,map.get("content").unwrap(),map.get("author_id").unwrap().parse().unwrap()).await{
                        Err(_)=>Response::BadRequest,
                        Ok(item)=>Response::Created(item.to_string())
                    }
                },
                "COMMENT"=>{
                    match CommentDAO::create(pool,map.get("content").unwrap(),map.get("author_id").unwrap().parse().unwrap(),map.get("article_id").unwrap().parse().unwrap()).await{
                        Err(_)=>Response::BadRequest,
                        Ok(item)=>Response::Created(item.to_string())
                    }
                },
                _ =>{Response::BadRequest}
            }
        }
        PATCH(field, map) => {
            match field.as_str() {//TODO!
                "USER"=>{
                    match UserDAO::patch(pool,map).await{
                        Ok(item) => {Response::OK(item.to_string())}
                        Err(_) => {Response::BadRequest}
                    }
                },
                "ARTICLE"=>{
                    match UserDAO::patch(pool,map).await{
                        Ok(item) => {Response::OK(item.to_string())}
                        Err(_) => {Response::BadRequest}
                    }
                },
                "COMMENT"=>{
                    match UserDAO::patch(pool,map).await{
                        Ok(item) => {Response::OK(item.to_string())}
                        Err(_) => {Response::BadRequest}
                    }
                }
                _=>Response::BadRequest,
            }
        }
        DELETE(field, _map) => {
            //TODO!
            match field.as_str() {
                _=>Response::BadRequest,
            }
        }
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
            //socket.write(&[i]).await?;
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
