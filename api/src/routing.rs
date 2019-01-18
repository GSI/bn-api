use actix_web::middleware::cors::CorsBuilder;
use actix_web::{http::Method, App, HttpResponse};
use controllers::*;
use server::AppState;

pub fn routes(app: &mut CorsBuilder<AppState>) -> App<AppState> {
    // Please try to keep in alphabetical order
    app.resource("/artists/search", |r| {
        r.method(Method::GET).with(artists::search);
    })
    .resource("/artists/{id}/toggle_privacy", |r| {
        r.method(Method::PUT).with(artists::toggle_privacy);
    })
    .resource("/artists/{id}", |r| {
        r.method(Method::GET).with(artists::show);
        r.method(Method::PUT).with(artists::update);
    })
    .resource("/artists", |r| {
        r.method(Method::GET).with(artists::index);
        r.method(Method::POST).with(artists::create);
    })
    .resource("/auth/token", |r| r.method(Method::POST).with(auth::token))
    .resource("/auth/token/refresh", |r| {
        r.method(Method::POST).with(auth::token_refresh)
    })
    .resource("/cart", |r| {
        r.method(Method::DELETE).with(cart::destroy);
        r.method(Method::POST).with(cart::update_cart);
        r.method(Method::PUT).with(cart::replace_cart);
        r.method(Method::GET).with(cart::show);
    })
    .resource("/cart/checkout", |r| {
        r.method(Method::POST).with(cart::checkout);
    })
    .resource("/codes/{id}", |r| {
        r.method(Method::GET).with(codes::show);
        r.method(Method::PUT).with(codes::update);
        r.method(Method::DELETE).with(codes::destroy);
    })
    .resource("/comps/{id}", |r| {
        r.method(Method::GET).with(comps::show);
        r.method(Method::PATCH).with(comps::update);
        r.method(Method::DELETE).with(comps::destroy);
    })
    .resource("/events", |r| {
        r.method(Method::GET).with(events::index);
        r.method(Method::POST).with(events::create);
    })
    .resource("/events/checkins", |r| {
        r.method(Method::GET).with(events::checkins);
    })
    .resource("/events/{id}", |r| {
        r.method(Method::GET).with(events::show);
        r.method(Method::PUT).with(events::update);
        r.method(Method::DELETE).with(events::cancel);
    })
    .resource("/events/{id}/artists", |r| {
        r.method(Method::POST).with(events::add_artist);
        r.method(Method::PUT).with(events::update_artists);
    })
    .resource("/events/{id}/codes", |r| {
        r.method(Method::GET).with(events::codes);
        r.method(Method::POST).with(codes::create);
    })
    .resource("/events/{id}/dashboard", |r| {
        r.method(Method::GET).with(events::dashboard);
    })
    .resource("/events/{id}/guests", |r| {
        r.method(Method::GET).with(events::guest_list);
    })
    .resource("/events/{id}/holds", |r| {
        r.method(Method::POST).with(holds::create);
        r.method(Method::GET).with(events::holds);
    })
    .resource("/events/{id}/fans", |r| {
        r.method(Method::GET).with(events::fans_index);
    })
    .resource("/events/{id}/interest", |r| {
        r.method(Method::GET).with(events::list_interested_users);
        r.method(Method::POST).with(events::add_interest);
        r.method(Method::DELETE).with(events::remove_interest);
    })
    .resource("/events/{id}/publish", |r| {
        r.method(Method::POST).with(events::publish);
    })
    .resource("/events/{id}/unpublish", |r| {
        r.method(Method::POST).with(events::unpublish);
    })
    .resource("/events/{id}/redeem/{ticket_instance_id}", |r| {
        r.method(Method::POST).with(events::redeem_ticket);
    })
    .resource("/events/{id}/tickets", |r| {
        r.method(Method::GET).with(tickets::index);
    })
    .resource("/events/{id}/ticket_types", |r| {
        r.method(Method::GET).with(ticket_types::index);
        r.method(Method::POST).with(ticket_types::create);
    })
    .resource("/events/{event_id}/ticket_types/{ticket_type_id}", |r| {
        r.method(Method::PATCH).with(ticket_types::update);
        r.method(Method::DELETE).with(ticket_types::cancel);
    })
    .resource("/external/facebook/web_login", |r| {
        r.method(Method::POST).with(external::facebook::web_login)
    })
    .resource("/invitations/{id}", |r| {
        r.method(Method::GET).with(organization_invites::view);
    })
    .resource("/invitations", |r| {
        r.method(Method::POST)
            .with(organization_invites::accept_request);
    })
    .resource("/ipns/globee", |r| {
        r.method(Method::POST).with(ipns::globee);
    })
    .resource("/holds/{id}/comps", |r| {
        r.method(Method::GET).with(comps::index);
        r.method(Method::POST).with(comps::create);
    })
    .resource("/holds/{id}/split", |r| {
        r.method(Method::POST).with(holds::split);
    })
    .resource("/holds/{id}", |r| {
        r.method(Method::PATCH).with(holds::update);
        r.method(Method::GET).with(holds::show);
        r.method(Method::DELETE).with(holds::destroy);
    })
    .resource("/orders", |r| {
        r.method(Method::GET).with(orders::index);
    })
    .resource("/orders/{id}/details", |r| {
        r.method(Method::GET).with(orders::details);
    })
    .resource("/orders/{id}/refund", |r| {
        r.method(Method::PATCH).with(orders::refund);
    })
    .resource("/orders/{id}/tickets", |r| {
        r.method(Method::GET).with(orders::tickets);
    })
    .resource("/orders/{id}", |r| {
        r.method(Method::GET).with(orders::show);
        r.method(Method::PATCH).with(orders::update);
    })
    .resource("/organizations/{id}/artists", |r| {
        r.method(Method::GET).with(artists::show_from_organizations);
        r.method(Method::POST).with(organizations::add_artist);
    })
    .resource("/organizations/{id}/events", |r| {
        r.method(Method::GET).with(events::show_from_organizations);
    })
    .resource("/organizations/{id}/fans/{user_id}/history", |r| {
        r.method(Method::GET).with(users::history);
    })
    .resource("/organizations/{id}/fans/{user_id}", |r| {
        r.method(Method::GET).with(users::profile);
    })
    .resource("/organizations/{id}/fee_schedule", |r| {
        r.method(Method::GET).with(organizations::show_fee_schedule);
        r.method(Method::POST).with(organizations::add_fee_schedule);
    })
    .resource("/organizations/{id}/fans", |r| {
        r.method(Method::GET).with(organizations::search_fans);
    })
    .resource("/organizations/{id}/invites/{invite_id}", |r| {
        r.method(Method::DELETE).with(organization_invites::destroy);
    })
    .resource("/organizations/{id}/invites", |r| {
        r.method(Method::GET).with(organization_invites::index);
        r.method(Method::POST).with(organization_invites::create);
    })
    .resource("/organizations/{id}/users", |r| {
        r.method(Method::POST)
            .with(organizations::add_or_replace_user);
        r.method(Method::PUT)
            .with(organizations::add_or_replace_user);
        r.method(Method::GET)
            .with(organizations::list_organization_members);
    })
    .resource("/organizations/{id}/users/{user_id}", |r| {
        r.method(Method::DELETE).with(organizations::remove_user);
    })
    .resource("/organizations/{id}/venues", |r| {
        r.method(Method::GET).with(venues::show_from_organizations);
        r.method(Method::POST).with(organizations::add_venue);
    })
    .resource("/organizations/{id}", |r| {
        r.method(Method::GET).with(organizations::show);
        r.method(Method::PATCH).with(organizations::update);
    })
    .resource("/organizations", |r| {
        r.method(Method::GET).with(organizations::index);
        r.method(Method::POST).with(organizations::create);
    })
    .resource("/password_reset", |r| {
        r.method(Method::POST).with(password_resets::create);
        r.method(Method::PUT).with(password_resets::update);
    })
    .resource("/payment_methods", |r| {
        r.method(Method::GET).with(payment_methods::index);
    })
    .resource("/redemption_codes/{code}", |r| {
        r.method(Method::GET).with(redemption_codes::show)
    })
    .resource("/regions/{id}", |r| {
        r.method(Method::GET).with(regions::show);
        r.method(Method::PUT).with(regions::update);
    })
    .resource("/regions", |r| {
        r.method(Method::GET).with(regions::index);
        r.method(Method::POST).with(regions::create)
    })
    .resource("/reports/{id}", |r| {
        r.method(Method::GET).with(reports::get_report);
    })
    .resource("/status", |r| {
        r.method(Method::GET).f(|_| HttpResponse::Ok())
    })
    .resource("/stages/{id}", |r| {
        r.method(Method::GET).with(stages::show);
        r.method(Method::PUT).with(stages::update);
        r.method(Method::DELETE).with(stages::delete);
    })
    .resource("/tickets/transfer", |r| {
        r.method(Method::POST).with(tickets::transfer_authorization);
    })
    .resource("/tickets/receive", |r| {
        r.method(Method::POST).with(tickets::receive_transfer);
    })
    .resource("/tickets/send", |r| {
        r.method(Method::POST)
            .with(tickets::send_via_email_or_phone);
    })
    .resource("/tickets/{id}", |r| {
        r.method(Method::GET).with(tickets::show);
    })
    .resource("/tickets", |r| {
        r.method(Method::GET).with(tickets::index);
    })
    .resource("/tickets/{id}/redeem", |r| {
        r.method(Method::GET).with(tickets::show_redeemable_ticket);
    })
    .resource("/users/me", |r| {
        r.method(Method::GET).with(users::current_user);
        r.method(Method::PUT).with(users::update_current_user);
    })
    .resource("/users/register", |r| {
        r.method(Method::POST).with(users::register)
    })
    .resource("/users/{id}/tokens", |r| {
        r.method(Method::GET)
            .with(users::show_push_notification_tokens_for_user_id);
    })
    .resource("/users/tokens", |r| {
        r.method(Method::GET)
            .with(users::show_push_notification_tokens);
        r.method(Method::POST)
            .with(users::add_push_notification_token);
    })
    .resource("/users/tokens/{id}", |r| {
        r.method(Method::DELETE)
            .with(users::remove_push_notification_token);
    })
    .resource("/users", |r| {
        r.method(Method::POST).with(users::register_and_login);
    })
    .resource("/users/{id}", |r| {
        r.method(Method::GET).with(users::show);
    })
    .resource("/user_invites", |r| {
        r.method(Method::POST).with(user_invites::create);
    })
    .resource("/users/{id}/organizations", |r| {
        r.method(Method::GET).with(users::list_organizations);
    })
    .resource("/venues/{id}/events", |r| {
        r.method(Method::GET).with(events::show_from_venues);
    })
    .resource("/venues/{id}/organizations", |r| {
        r.method(Method::POST).with(venues::add_to_organization);
    })
    .resource("/venues/{id}/stages", |r| {
        r.method(Method::POST).with(stages::create);
        r.method(Method::GET).with(stages::index);
    })
    .resource("/venues/{id}/toggle_privacy", |r| {
        r.method(Method::PUT).with(venues::toggle_privacy);
    })
    .resource("/venues/{id}", |r| {
        r.method(Method::GET).with(venues::show);
        r.method(Method::PUT).with(venues::update);
    })
    .resource("/venues", |r| {
        r.method(Method::GET).with(venues::index);
        r.method(Method::POST).with(venues::create);
    })
    .register()
    .default_resource(|r| {
        r.method(Method::GET)
            .f(|_req| HttpResponse::NotFound().json(json!({"error": "Not found"})));
    })
}
