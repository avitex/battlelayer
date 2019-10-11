use std::net::SocketAddrV4;
use crate::conn::Word;

/// A password is from 0 up to 16 characters in length, inclusive.
// abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789
pub struct Password(Word);

/// A stream of hexadecimal digits.
/// The stream must always contain an even number of digits.
// 0123456789ABCDEF
pub struct HexString(Word);

/// A filename is from 1 up to 240 characters in length, inclusive.
// abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._-
pub struct Filename(Word);

/// A clan tag is from 0 to an unknown number of characters in length.
/// At the time of writing, it is unclear which the allowed characters are.
pub struct Clantag(Word);

/// The "player name" (referred to as "Soldier name" in-game) is the persona
/// name which the player chose when logging in to EA Online.
/// The exact specification of a player name (length, valid characters, etc.)
/// is currently unclear.
pub struct PlayerName(Word);

/// The GUID is a unique identifier for a player.
/// It is 35 characters long, consists of the prefix "EA_" immediately
/// followed by a 32-character HexString.
pub struct PlayerGuid(Word);

/// An integer. Team 0 is neutral. 
/// Depending on gamemode, there are up to 16 non-neutral teams, numbered 1-16.
pub struct TeamId(u32);

/// An integer. Squad 0 is "no squad".
/// Depending on gamemode, there are up to 32 squads numbered 1-32.
/// Note that squad IDs are local within each team; that is, to uniquely 
/// identify a squad you need to specify both a Team ID and a Squad ID.
pub struct SquadId(u32);

/// Several commands – such as `admin.listPlayers` – take a player
/// subset as argument.
pub enum PlayerSubset {
    /// All players on the server
    All,
    /// All players in the specified team
    Team(TeamId),
    /// All players in the specified team and squad
    Squad(TeamId, SquadId),
    /// One specific player
    Player(PlayerName),
}

/// Some commands, such as bans, take a timeout as argument.
pub enum Timeout {
    /// Permanent
    Permanent,
    /// Number of rounds
    Rounds(u32),
    /// Number of seconds
    Seconds(u32)
}

/// Some commands, such as bans, take an id-type as argument
pub enum PlayerId {
    /// Soldier name
    Name(PlayerName),
    /// IP address
    Ip(SocketAddrV4),
    /// Player’s GUID
    Guid(PlayerGuid),
}

/// The standard set of info for a group of players contains a lot
/// of different fields. To reduce the risk of having to do
/// backwards-incompatible changes to the protocol, the player info 
/// block includes some formatting information.
pub struct PlayerInfo {
    /// Player name
    pub name: PlayerName,
    /// Player's GUID
    pub guid: PlayerGuid,
    /// Player's current team
    pub team_id: TeamId,
    /// Player's current squad
    pub squad_id: SquadId,
    /// Number of kills, as shown in the in-game scoreboard
    pub kills: u32,
    /// Number of deaths, as shown in the in-game scoreboard
    pub deaths: u32,
    /// Score, as shown in the in-game scoreboard
    pub score: u32,
    /// The rank of the player
    pub rank: u32,
    /// Ping between the server and player
    pub ping: u32,
}

/// This describes the number of tickets, or kills,
/// for each team in the current round.
pub struct TeamScores {
    /// Score for all teams
    pub score: Vec<u32>,
    /// When any team reaches this score, the match ends
    pub target_score: u32,
}

/// This describes the set of maps which the server rotates through.
pub struct MapList {
    pub maps: Vec<MapListItem>,
}

pub struct MapListItem {
    /// Number of words per map
    pub map_name: String,
    /// Name of game mode
    pub game_mode: String,
    /// Number of rounds to play on map before switching
    pub rounds: u32,
    /// Other words if extended
    pub words: Vec<Word>,
}