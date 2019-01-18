table! {
    artists (id) {
        id -> Uuid,
        organization_id -> Nullable<Uuid>,
        is_private -> Bool,
        name -> Text,
        bio -> Text,
        image_url -> Nullable<Text>,
        thumb_image_url -> Nullable<Text>,
        website_url -> Nullable<Text>,
        youtube_video_urls -> Array<Text>,
        facebook_username -> Nullable<Text>,
        instagram_username -> Nullable<Text>,
        snapchat_username -> Nullable<Text>,
        soundcloud_username -> Nullable<Text>,
        bandcamp_username -> Nullable<Text>,
        spotify_id -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        other_image_urls -> Nullable<Array<Text>>,
    }
}

table! {
    assets (id) {
        id -> Uuid,
        ticket_type_id -> Uuid,
        blockchain_name -> Text,
        blockchain_asset_id -> Nullable<Text>,
        status -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    codes (id) {
        id -> Uuid,
        name -> Text,
        event_id -> Uuid,
        code_type -> Text,
        redemption_code -> Text,
        max_uses -> Int8,
        discount_in_cents -> Nullable<Int8>,
        start_date -> Timestamp,
        end_date -> Timestamp,
        max_tickets_per_user -> Nullable<Int8>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    domain_actions (id) {
        id -> Uuid,
        domain_event_id -> Nullable<Uuid>,
        domain_action_type -> Text,
        communication_channel_type -> Nullable<Text>,
        payload -> Json,
        main_table -> Nullable<Text>,
        main_table_id -> Nullable<Uuid>,
        scheduled_at -> Timestamp,
        expires_at -> Timestamp,
        last_attempted_at -> Nullable<Timestamp>,
        attempt_count -> Int8,
        max_attempt_count -> Int8,
        status -> Text,
        last_failure_reason -> Nullable<Text>,
        blocked_until -> Timestamp,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    domain_events (id) {
        id -> Uuid,
        event_type -> Text,
        display_text -> Text,
        event_data -> Nullable<Json>,
        main_table -> Text,
        main_id -> Nullable<Uuid>,
        published_at -> Nullable<Timestamp>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        user_id -> Nullable<Uuid>,
    }
}

table! {
    event_artists (id) {
        id -> Uuid,
        event_id -> Uuid,
        artist_id -> Uuid,
        rank -> Int4,
        set_time -> Nullable<Timestamp>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        importance -> Int4,
        stage_id -> Nullable<Uuid>,
    }
}

table! {
    event_interest (id) {
        id -> Uuid,
        event_id -> Uuid,
        user_id -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    events (id) {
        id -> Uuid,
        name -> Text,
        organization_id -> Uuid,
        venue_id -> Nullable<Uuid>,
        created_at -> Timestamp,
        event_start -> Nullable<Timestamp>,
        door_time -> Nullable<Timestamp>,
        status -> Text,
        publish_date -> Nullable<Timestamp>,
        redeem_date -> Nullable<Timestamp>,
        fee_in_cents -> Int8,
        promo_image_url -> Nullable<Text>,
        additional_info -> Nullable<Text>,
        age_limit -> Nullable<Int4>,
        top_line_info -> Nullable<Varchar>,
        cancelled_at -> Nullable<Timestamp>,
        updated_at -> Timestamp,
        min_ticket_price_cache -> Nullable<Int8>,
        max_ticket_price_cache -> Nullable<Int8>,
        video_url -> Nullable<Text>,
        is_external -> Bool,
        external_url -> Nullable<Text>,
        override_status -> Nullable<Text>,
        client_fee_in_cents -> Int8,
        company_fee_in_cents -> Int8,
        settlement_amount_in_cents -> Nullable<Int8>,
        event_end -> Nullable<Timestamp>,
        sendgrid_list_id -> Nullable<Int8>,
    }
}

table! {
    external_logins (id) {
        id -> Uuid,
        user_id -> Uuid,
        created_at -> Timestamp,
        site -> Text,
        access_token -> Text,
        external_user_id -> Text,
        updated_at -> Timestamp,
    }
}

table! {
    fee_schedule_ranges (id) {
        id -> Uuid,
        fee_schedule_id -> Uuid,
        min_price_in_cents -> Int8,
        fee_in_cents -> Int8,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        company_fee_in_cents -> Int8,
        client_fee_in_cents -> Int8,
    }
}

table! {
    fee_schedules (id) {
        id -> Uuid,
        name -> Text,
        version -> Int2,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    holds (id) {
        id -> Uuid,
        name -> Text,
        parent_hold_id -> Nullable<Uuid>,
        event_id -> Uuid,
        redemption_code -> Text,
        discount_in_cents -> Nullable<Int8>,
        end_at -> Nullable<Timestamp>,
        max_per_order -> Nullable<Int8>,
        hold_type -> Text,
        ticket_type_id -> Uuid,
        email -> Nullable<Text>,
        phone -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    order_items (id) {
        id -> Uuid,
        order_id -> Uuid,
        item_type -> Text,
        ticket_type_id -> Nullable<Uuid>,
        event_id -> Nullable<Uuid>,
        quantity -> Int8,
        unit_price_in_cents -> Int8,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        ticket_pricing_id -> Nullable<Uuid>,
        fee_schedule_range_id -> Nullable<Uuid>,
        parent_id -> Nullable<Uuid>,
        hold_id -> Nullable<Uuid>,
        code_id -> Nullable<Uuid>,
        company_fee_in_cents -> Int8,
        client_fee_in_cents -> Int8,
        refunded_quantity -> Int8,
    }
}

table! {
    orders (id) {
        id -> Uuid,
        user_id -> Uuid,
        status -> Text,
        order_type -> Text,
        order_date -> Timestamp,
        expires_at -> Nullable<Timestamp>,
        version -> Int8,
        note -> Nullable<Text>,
        on_behalf_of_user_id -> Nullable<Uuid>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        paid_at -> Nullable<Timestamp>,
        box_office_pricing -> Bool,
    }
}

table! {
    organization_invites (id) {
        id -> Uuid,
        organization_id -> Uuid,
        inviter_id -> Uuid,
        user_email -> Text,
        created_at -> Timestamp,
        security_token -> Nullable<Uuid>,
        user_id -> Nullable<Uuid>,
        accepted -> Nullable<Int2>,
        updated_at -> Timestamp,
        sent_invite -> Bool,
        roles -> Array<Text>,
    }
}

table! {
    organizations (id) {
        id -> Uuid,
        name -> Text,
        address -> Nullable<Text>,
        city -> Nullable<Text>,
        state -> Nullable<Text>,
        country -> Nullable<Text>,
        postal_code -> Nullable<Text>,
        phone -> Nullable<Text>,
        event_fee_in_cents -> Int8,
        sendgrid_api_key -> Nullable<Text>,
        google_ga_key -> Nullable<Text>,
        facebook_pixel_key -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        fee_schedule_id -> Uuid,
        client_event_fee_in_cents -> Int8,
        company_event_fee_in_cents -> Int8,
    }
}

table! {
    organization_users (id) {
        id -> Uuid,
        organization_id -> Uuid,
        user_id -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        role -> Array<Text>,
    }
}

table! {
    payment_methods (id) {
        id -> Uuid,
        user_id -> Uuid,
        name -> Text,
        is_default -> Bool,
        provider -> Text,
        provider_data -> Json,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    payments (id) {
        id -> Uuid,
        order_id -> Uuid,
        created_by -> Uuid,
        status -> Text,
        payment_method -> Text,
        amount -> Int8,
        provider -> Text,
        external_reference -> Nullable<Text>,
        raw_data -> Nullable<Json>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    push_notification_tokens (id) {
        id -> Uuid,
        user_id -> Uuid,
        token_source -> Text,
        token -> Text,
        last_notification_at -> Nullable<Timestamp>,
        created_at -> Timestamp,
    }
}

table! {
    refunded_tickets (id) {
        id -> Uuid,
        order_item_id -> Uuid,
        ticket_instance_id -> Uuid,
        fee_refunded_at -> Nullable<Timestamp>,
        ticket_refunded_at -> Nullable<Timestamp>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    regions (id) {
        id -> Uuid,
        name -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    stages (id) {
        id -> Uuid,
        venue_id -> Uuid,
        name -> Text,
        description -> Nullable<Text>,
        capacity -> Nullable<Int8>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    ticket_instances (id) {
        id -> Uuid,
        asset_id -> Uuid,
        token_id -> Int4,
        hold_id -> Nullable<Uuid>,
        order_item_id -> Nullable<Uuid>,
        wallet_id -> Uuid,
        reserved_until -> Nullable<Timestamp>,
        redeem_key -> Nullable<Text>,
        transfer_key -> Nullable<Uuid>,
        transfer_expiry_date -> Nullable<Timestamp>,
        status -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    ticket_pricing (id) {
        id -> Uuid,
        ticket_type_id -> Uuid,
        name -> Text,
        status -> Text,
        price_in_cents -> Int8,
        start_date -> Timestamp,
        end_date -> Timestamp,
        is_box_office_only -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    ticket_type_codes (id) {
        id -> Uuid,
        ticket_type_id -> Uuid,
        code_id -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    ticket_types (id) {
        id -> Uuid,
        event_id -> Uuid,
        name -> Text,
        description -> Nullable<Text>,
        status -> Text,
        start_date -> Timestamp,
        end_date -> Timestamp,
        increment -> Int4,
        limit_per_person -> Int4,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        price_in_cents -> Int8,
        cancelled_at -> Nullable<Timestamp>,
    }
}

table! {
    users (id) {
        id -> Uuid,
        first_name -> Nullable<Text>,
        last_name -> Nullable<Text>,
        email -> Nullable<Text>,
        phone -> Nullable<Text>,
        profile_pic_url -> Nullable<Text>,
        thumb_profile_pic_url -> Nullable<Text>,
        cover_photo_url -> Nullable<Text>,
        hashed_pw -> Text,
        password_modified_at -> Timestamp,
        created_at -> Timestamp,
        last_used -> Nullable<Timestamp>,
        active -> Bool,
        role -> Array<Text>,
        password_reset_token -> Nullable<Uuid>,
        password_reset_requested_at -> Nullable<Timestamp>,
        updated_at -> Timestamp,
        last_cart_id -> Nullable<Uuid>,
        accepted_terms_date -> Nullable<Timestamp>,
        invited_at -> Nullable<Timestamp>,
    }
}

table! {
    venues (id) {
        id -> Uuid,
        region_id -> Uuid,
        organization_id -> Nullable<Uuid>,
        is_private -> Bool,
        name -> Text,
        address -> Text,
        city -> Text,
        state -> Text,
        country -> Text,
        postal_code -> Text,
        phone -> Nullable<Text>,
        promo_image_url -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        google_place_id -> Nullable<Text>,
        latitude -> Nullable<Float8>,
        longitude -> Nullable<Float8>,
        timezone -> Nullable<Text>,
    }
}

table! {
    wallets (id) {
        id -> Uuid,
        user_id -> Nullable<Uuid>,
        organization_id -> Nullable<Uuid>,
        name -> Text,
        secret_key -> Text,
        public_key -> Text,
        default_flag -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

joinable!(artists -> organizations (organization_id));
joinable!(assets -> ticket_types (ticket_type_id));
joinable!(codes -> events (event_id));
joinable!(domain_actions -> domain_events (domain_event_id));
joinable!(domain_events -> users (user_id));
joinable!(event_artists -> artists (artist_id));
joinable!(event_artists -> events (event_id));
joinable!(event_artists -> stages (stage_id));
joinable!(event_interest -> events (event_id));
joinable!(event_interest -> users (user_id));
joinable!(events -> organizations (organization_id));
joinable!(events -> venues (venue_id));
joinable!(external_logins -> users (user_id));
joinable!(fee_schedule_ranges -> fee_schedules (fee_schedule_id));
joinable!(holds -> events (event_id));
joinable!(holds -> ticket_types (ticket_type_id));
joinable!(order_items -> codes (code_id));
joinable!(order_items -> events (event_id));
joinable!(order_items -> fee_schedule_ranges (fee_schedule_range_id));
joinable!(order_items -> holds (hold_id));
joinable!(order_items -> orders (order_id));
joinable!(order_items -> ticket_pricing (ticket_pricing_id));
joinable!(order_items -> ticket_types (ticket_type_id));
joinable!(organization_invites -> organizations (organization_id));
joinable!(organization_users -> organizations (organization_id));
joinable!(organization_users -> users (user_id));
joinable!(organizations -> fee_schedules (fee_schedule_id));
joinable!(payment_methods -> users (user_id));
joinable!(payments -> orders (order_id));
joinable!(payments -> users (created_by));
joinable!(push_notification_tokens -> users (user_id));
joinable!(refunded_tickets -> order_items (order_item_id));
joinable!(refunded_tickets -> ticket_instances (ticket_instance_id));
joinable!(ticket_instances -> assets (asset_id));
joinable!(ticket_instances -> holds (hold_id));
joinable!(ticket_instances -> order_items (order_item_id));
joinable!(ticket_instances -> wallets (wallet_id));
joinable!(ticket_pricing -> ticket_types (ticket_type_id));
joinable!(ticket_type_codes -> codes (code_id));
joinable!(ticket_type_codes -> ticket_types (ticket_type_id));
joinable!(ticket_types -> events (event_id));
joinable!(venues -> organizations (organization_id));
joinable!(venues -> regions (region_id));
joinable!(wallets -> organizations (organization_id));
joinable!(wallets -> users (user_id));

allow_tables_to_appear_in_same_query!(
    artists,
    assets,
    codes,
    domain_actions,
    domain_events,
    event_artists,
    event_interest,
    events,
    external_logins,
    fee_schedule_ranges,
    fee_schedules,
    holds,
    order_items,
    orders,
    organization_invites,
    organizations,
    organization_users,
    payment_methods,
    payments,
    push_notification_tokens,
    refunded_tickets,
    regions,
    stages,
    ticket_instances,
    ticket_pricing,
    ticket_type_codes,
    ticket_types,
    users,
    venues,
    wallets,
);
