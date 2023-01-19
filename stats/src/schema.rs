// @generated automatically by Diesel CLI.

diesel::table! {
    categories (id) {
        id -> Integer,
        name -> Nullable<Varchar>,
    }
}

diesel::table! {
    championships (id) {
        id -> Integer,
        name -> Varchar,
        begin -> Date,
        end -> Date,
        point_win -> Integer,
        point_draw -> Integer,
        point_loss -> Integer,
        category_id -> Integer,
        show_country -> Bool,
        // created_at -> Datetime,
        // updated_at -> Datetime,
        region -> Integer,
        region_name -> Nullable<Text>,
    }
}

diesel::table! {
    comments (id) {
        id -> Integer,
        title -> Nullable<Varchar>,
        comment -> Nullable<Text>,
        created_at -> Datetime,
        commentable_id -> Integer,
        commentable_type -> Varchar,
        user_id -> Integer,
    }
}

diesel::table! {
    game_versions (id) {
        id -> Integer,
        game_id -> Nullable<Integer>,
        version -> Nullable<Integer>,
        home_id -> Nullable<Integer>,
        away_id -> Nullable<Integer>,
        phase_id -> Nullable<Integer>,
        round -> Nullable<Integer>,
        attendance -> Nullable<Integer>,
        stadium_id -> Nullable<Integer>,
        referee_id -> Nullable<Integer>,
        home_score -> Nullable<Integer>,
        away_score -> Nullable<Integer>,
        home_pen -> Nullable<Integer>,
        away_pen -> Nullable<Integer>,
        played -> Nullable<Bool>,
        updater_id -> Integer,
        updated_at -> Datetime,
        home_aet -> Nullable<Integer>,
        away_aet -> Nullable<Integer>,
        date -> Datetime,
        has_time -> Nullable<Bool>,
        home_field -> Integer,
        home_importance -> Nullable<Float>,
        away_importance -> Nullable<Float>,
        soccerway_id -> Nullable<Varchar>,
    }
}

diesel::table! {
    games (id) {
        id -> Integer,
        home_id -> Integer,
        away_id -> Integer,
        phase_id -> Integer,
        round -> Nullable<Integer>,
        attendance -> Nullable<Integer>,
        stadium_id -> Nullable<Integer>,
        referee_id -> Nullable<Integer>,
        home_score -> Integer,
        away_score -> Integer,
        home_pen -> Nullable<Integer>,
        away_pen -> Nullable<Integer>,
        played -> Bool,
        version -> Nullable<Integer>,
        updater_id -> Integer,
        updated_at -> Datetime,
        home_aet -> Nullable<Integer>,
        away_aet -> Nullable<Integer>,
        date -> Datetime,
        has_time -> Nullable<Bool>,
        home_field -> Integer,
        home_importance -> Nullable<Float>,
        away_importance -> Nullable<Float>,
        soccerway_id -> Nullable<Varchar>,
    }
}

diesel::table! {
    goals (id) {
        id -> Integer,
        player_id -> Integer,
        game_id -> Nullable<Integer>,
        team_id -> Integer,
        time -> Integer,
        penalty -> Bool,
        own_goal -> Bool,
        aet -> Bool,
        created_at -> Datetime,
        updated_at -> Datetime,
    }
}

diesel::table! {
    groups (id) {
        id -> Integer,
        phase_id -> Integer,
        name -> Varchar,
        promoted -> Integer,
        relegated -> Integer,
        odds_progress -> Nullable<Integer>,
        created_at -> Datetime,
        updated_at -> Datetime,
        zones -> Nullable<Text>,
    }
}

diesel::table! {
    historical_ratings (id) {
        id -> Integer,
        team_id -> Integer,
        rating -> Float,
        measure_date -> Date,
        off_rating -> Float,
        def_rating -> Float,
    }
}

diesel::table! {
    open_id_authentication_associations (id) {
        id -> Integer,
        issued -> Nullable<Integer>,
        lifetime -> Nullable<Integer>,
        handle -> Nullable<Varchar>,
        assoc_type -> Nullable<Varchar>,
        server_url -> Nullable<Blob>,
        secret -> Nullable<Blob>,
    }
}

diesel::table! {
    open_id_authentication_nonces (id) {
        id -> Integer,
        timestamp -> Integer,
        server_url -> Nullable<Varchar>,
        salt -> Varchar,
    }
}

diesel::table! {
    phases (id) {
        id -> Integer,
        championship_id -> Integer,
        name -> Varchar,
        order_by -> Integer,
        sort -> Varchar,
        bonus_points -> Integer,
        bonus_points_threshold -> Integer,
        // created_at -> Datetime,
        // updated_at -> Datetime,
    }
}

diesel::table! {
    player_games (id) {
        id -> Integer,
        player_id -> Integer,
        game_id -> Integer,
        team_id -> Integer,
        on -> Integer,
        off -> Integer,
        yellow -> Bool,
        red -> Bool,
        off_rating -> Nullable<Float>,
        def_rating -> Nullable<Float>,
    }
}

diesel::table! {
    players (id) {
        id -> Integer,
        name -> Varchar,
        position -> Nullable<Varchar>,
        birth -> Nullable<Date>,
        country -> Nullable<Varchar>,
        full_name -> Nullable<Varchar>,
        created_at -> Datetime,
        updated_at -> Datetime,
        soccerway_id -> Nullable<Varchar>,
        rating -> Nullable<Float>,
        off_rating -> Nullable<Float>,
        def_rating -> Nullable<Float>,
    }
}

diesel::table! {
    referee_champs (id) {
        id -> Integer,
        referee_id -> Integer,
        championship_id -> Integer,
    }
}

diesel::table! {
    referees (id) {
        id -> Integer,
        name -> Varchar,
        location -> Nullable<Varchar>,
        created_at -> Datetime,
        updated_at -> Datetime,
    }
}

diesel::table! {
    roles (id) {
        id -> Integer,
        name -> Nullable<Varchar>,
    }
}

diesel::table! {
    stadia (id) {
        id -> Integer,
        name -> Varchar,
        full_name -> Nullable<Varchar>,
        city -> Nullable<Varchar>,
        country -> Nullable<Varchar>,
        created_at -> Datetime,
        updated_at -> Datetime,
    }
}

diesel::table! {
    team_groups (id) {
        id -> Integer,
        group_id -> Integer,
        team_id -> Integer,
        add_sub -> Integer,
        bias -> Integer,
        comment -> Nullable<Text>,
        created_at -> Datetime,
        updated_at -> Datetime,
        odds -> Nullable<Text>,
    }
}

diesel::table! {
    team_players (id) {
        id -> Integer,
        team_id -> Integer,
        player_id -> Integer,
        championship_id -> Integer,
        created_at -> Datetime,
        updated_at -> Datetime,
    }
}

diesel::table! {
    teams (id) {
        id -> Integer,
        name -> Varchar,
        country -> Varchar,
        legacy_logo -> Nullable<Varchar>,
        city -> Nullable<Varchar>,
        stadium_id -> Nullable<Integer>,
        foundation -> Nullable<Date>,
        full_name -> Nullable<Varchar>,
        logo_file_name -> Nullable<Varchar>,
        logo_content_type -> Nullable<Varchar>,
        logo_file_size -> Nullable<Integer>,
        logo_updated_at -> Nullable<Datetime>,
        created_at -> Datetime,
        updated_at -> Datetime,
        rating -> Nullable<Float>,
        off_rating -> Nullable<Float>,
        def_rating -> Nullable<Float>,
        team_type -> Integer,
    }
}

diesel::table! {
    users (id) {
        id -> Integer,
        login -> Nullable<Varchar>,
        email -> Nullable<Varchar>,
        crypted_password -> Nullable<Varchar>,
        salt -> Nullable<Varchar>,
        created_at -> Nullable<Datetime>,
        updated_at -> Nullable<Datetime>,
        remember_token -> Nullable<Varchar>,
        remember_token_expires_at -> Nullable<Datetime>,
        identity_url -> Nullable<Varchar>,
        name -> Nullable<Varchar>,
        location -> Nullable<Varchar>,
        birthday -> Nullable<Date>,
        about_me -> Nullable<Text>,
        last_login -> Nullable<Datetime>,
        avatar_file_name -> Nullable<Varchar>,
        avatar_content_type -> Nullable<Varchar>,
        avatar_file_size -> Nullable<Integer>,
        avatar_updated_at -> Nullable<Datetime>,
        openid_connect_token -> Nullable<Varchar>,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    categories,
    championships,
    comments,
    game_versions,
    games,
    goals,
    groups,
    historical_ratings,
    open_id_authentication_associations,
    open_id_authentication_nonces,
    phases,
    player_games,
    players,
    referee_champs,
    referees,
    roles,
    stadia,
    team_groups,
    team_players,
    teams,
    users,
);
