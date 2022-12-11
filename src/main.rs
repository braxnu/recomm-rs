#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use std::{sync::Mutex, collections::HashMap};
use serde::{Serialize, Deserialize};
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder, delete};
use mongodb::{Client, bson::doc, Database, options::FindOptions};
use env_logger;
use serde_json::json;

type ProductId = String;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct Product {
    id: ProductId,
    name: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct OrderItem {
    product: Product,
    quantity: u16,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct Order {
    id: String,
    items: Vec<OrderItem>,
}

struct AppState {
    orders: Vec<Order>,
}

#[get("/")]
async fn get_index() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

#[post("/orders")]
async fn post_order(state: web::Data<Mutex<AppState>>, order: web::Json<Order>) -> impl Responder {
    let order_id = &order.id;

    if order.items.len() == 0 {
        return HttpResponse::BadRequest().json(json!({
            "success": false,
            "reason": "no items",
        }));
    }

    let orders = &mut state.lock().unwrap().orders;

    if orders.iter().any(|o| { &o.id == order_id }) {
        return HttpResponse::Conflict().json(json!({
            "success": false,
            "reason": format!("order {} already exists", order_id),
        }));
    }

    orders.push(order.into_inner());

    HttpResponse::Ok().json(json!({
        "success": true,
    }))
}

#[get("/orders")]
async fn get_orders(state: web::Data<Mutex<AppState>>) -> impl Responder {
    let orders = & state.lock().unwrap().orders;

    HttpResponse::Ok().json(orders)
}

#[delete("/orders")]
async fn delete_orders(state: web::Data<Mutex<AppState>>) -> impl Responder {
    let s = &mut state.lock().unwrap();

    s.orders = vec![];

    HttpResponse::Accepted().json(json!({
        "success": true,
    }))
}

#[get("/products/{product_id}/bought_together")]
async fn get_bought_together(
    state: web::Data<Mutex<AppState>>,
    path: web::Path<ProductId>
) -> impl Responder {
    let product_id = path.into_inner();
    let mut product_count_map: HashMap<ProductId, u32> = HashMap::new();
    let orders = & state.lock().unwrap().orders;

    for o in orders {
        if o.items.iter().any(|i| { i.product.id == product_id }) {
            for p in o.items.iter() {
                product_count_map.insert(
                    p.product.id.clone(),
                    product_count_map.get(&p.product.id).or(Some(&0)).unwrap() + 1
                );
            }
        }
    }

    let mut entries: Vec<(ProductId, u32)> =
        product_count_map
            .into_iter()
            .collect();

    let serached_product_index = entries.iter()
        .position(|p| {
            p.0 == product_id
        })
        .unwrap();

    entries.remove(serached_product_index);
    entries.sort_by_key(|e| { e.1.clone() });
    entries.reverse();

    let last_index = std::cmp::min(10, entries.len());

    let product_list: Vec<ProductId> = entries[0..last_index].iter()
        .map(|e| { e.0.clone() })
        .collect();

    HttpResponse::Ok().json(product_list)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let state = web::Data::new(
        Mutex::new(
            AppState {
                orders: vec![],
            }
        )
    );

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .service(get_index)
            .service(get_bought_together)
            .service(get_orders)
            .service(delete_orders)
            .service(post_order)
    })
        .bind(("127.0.0.1", 4600))?
        .run()
        .await
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use actix_web::{
        http::header::ContentType,
        test::{self, read_body_json},
        web::{self, Bytes},
        App,
        body::{MessageBody, BoxBody}
    };
    use serde_json::json;
    use crate::{
        AppState,
        Order,
        OrderItem,
        Product,
        delete_orders,
        post_order, get_bought_together,
    };

    use super::{get_orders};

    #[actix_web::test]
    async fn test_get_orders() {
        let orders: Vec<Order> = vec![
            Order {
                id: "aaa".to_string(),
                items: vec![
                    OrderItem {
                        product: Product {
                            id: "ccc".to_string(),
                            name: "box".to_string(),
                        },
                        quantity: 3,
                    }
                ],
            }
        ];

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(
                    Mutex::new(
                        AppState {
                            orders: orders.clone(),
                        }
                    )
                ))
                .service(get_orders)
        ).await;

        let req = test::TestRequest::default()
            .uri("/orders")
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert!(resp.status().is_success());

        let body: Vec<Order> = read_body_json(resp).await;

        assert!(body[0] == orders[0]);
    }

    #[actix_web::test]
    async fn test_delete_orders() {
        let orders: Vec<Order> = vec![
            Order {
                id: "aaa".to_string(),
                items: vec![
                    OrderItem {
                        product: Product {
                            id: "ccc".to_string(),
                            name: "box".to_string(),
                        },
                        quantity: 3,
                    }
                ],
            },
        ];

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(
                    Mutex::new(
                        AppState {
                            orders: orders.clone(),
                        }
                    )
                ))
                .service(get_orders)
                .service(delete_orders)
        ).await;

        let req = test::TestRequest::delete()
            .uri("/orders")
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert!(resp.status().is_success());

        let req = test::TestRequest::get()
            .uri("/orders")
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert!(resp.status().is_success());

        let body: Vec<Order> = read_body_json(resp).await;

        assert!(body == vec![]);
    }

    #[actix_web::test]
    async fn test_post_order() {
        let orders: Vec<Order> = vec![];

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(
                    Mutex::new(
                        AppState {
                            orders: orders.clone(),
                        }
                    )
                ))
                .service(get_orders)
                .service(post_order)
        ).await;

        let req = test::TestRequest::post()
            .uri("/orders")
            .insert_header(ContentType::json())
            .set_payload(
                json!({
                    "id": "aaa",
                    "items": [
                        {
                            "product": {
                                "id": "ccc",
                                "name": "box",
                            },
                            "quantity": 3,
                        }
                    ],
                }).to_string()
            )
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert!(resp.status().is_success());

        let req = test::TestRequest::get()
            .uri("/orders")
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert!(resp.status().is_success());

        let body: Vec<Order> = read_body_json(resp).await;

        assert!(body == vec![
            Order {
                id: "aaa".to_string(),
                items: vec![
                    OrderItem {
                        product: Product {
                            id: "ccc".to_string(),
                            name: "box".to_string(),
                        },
                        quantity: 3,
                    }
                ],
            },
        ]);
    }

    #[actix_web::test]
    async fn test_bought_together() {
        let orders: Vec<Order> = vec![
            Order {
                id: "o-1".to_string(),
                items: vec![
                    OrderItem {
                        product: Product {
                            id: "sample".to_string(),
                            name: "box".to_string(),
                        },
                        quantity: 3,
                    },
                    OrderItem {
                        product: Product {
                            id: "aaa".to_string(),
                            name: "box".to_string(),
                        },
                        quantity: 3,
                    },
                    OrderItem {
                        product: Product {
                            id: "bbb".to_string(),
                            name: "box".to_string(),
                        },
                        quantity: 3,
                    },
                    OrderItem {
                        product: Product {
                            id: "ccc".to_string(),
                            name: "box".to_string(),
                        },
                        quantity: 3,
                    },
                ],
            },
            Order {
                id: "o-2".to_string(),
                items: vec![
                    OrderItem {
                        product: Product {
                            id: "sample".to_string(),
                            name: "box".to_string(),
                        },
                        quantity: 3,
                    },
                    OrderItem {
                        product: Product {
                            id: "aaa".to_string(),
                            name: "box".to_string(),
                        },
                        quantity: 3,
                    },
                    OrderItem {
                        product: Product {
                            id: "ccc".to_string(),
                            name: "box".to_string(),
                        },
                        quantity: 3,
                    },
                ],
            },
            Order {
                id: "o-3".to_string(),
                items: vec![
                    OrderItem {
                        product: Product {
                            id: "sample".to_string(),
                            name: "box".to_string(),
                        },
                        quantity: 3,
                    },
                    OrderItem {
                        product: Product {
                            id: "ccc".to_string(),
                            name: "box".to_string(),
                        },
                        quantity: 3,
                    },
                ],
            },
            Order {
                id: "o-4".to_string(),
                items: vec![
                    OrderItem {
                        product: Product {
                            id: "ddd".to_string(),
                            name: "box".to_string(),
                        },
                        quantity: 3,
                    },
                ],
            },
            Order {
                id: "o-5".to_string(),
                items: vec![
                    OrderItem {
                        product: Product {
                            id: "ddd".to_string(),
                            name: "box".to_string(),
                        },
                        quantity: 3,
                    },
                ],
            },
        ];

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(
                    Mutex::new(
                        AppState {
                            orders: orders.clone(),
                        }
                    )
                ))
                .service(get_bought_together)
        ).await;

        let uri = format!("/products/{}/bought_together", "sample");

        let req = test::TestRequest::get()
            .uri(uri.as_str())
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert!(resp.status().is_success());

        let body: Vec<String> = read_body_json(resp).await;

        assert_eq!(body, vec![
            "ccc".to_string(),
            "aaa".to_string(),
            "bbb".to_string(),
        ]);

        assert!(!body.contains(&"ddd".to_string()));
    }
}
