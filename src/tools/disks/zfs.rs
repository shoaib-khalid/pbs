use std::path::PathBuf;
use std::collections::{HashMap, HashSet};
use std::os::unix::fs::MetadataExt;

use anyhow::{bail, Error};
use lazy_static::lazy_static;

use nom::{
    error::VerboseError,
    bytes::complete::{take_while, take_while1, take_till, take_till1},
    combinator::{map_res, all_consuming, recognize, opt},
    sequence::{preceded, tuple},
    character::complete::{space1, digit1, char, line_ending},
    multi::{many0},
};

use super::*;

lazy_static!{
    static ref ZFS_UUIDS: HashSet<&'static str> = {
        let mut set = HashSet::new();
	set.insert("6a898cc3-1dd2-11b2-99a6-080020736631"); // apple
	set.insert("516e7cba-6ecf-11d6-8ff8-00022d09712b"); // bsd
        set
    };
}

type IResult<I, O, E = VerboseError<I>> = Result<(I, O), nom::Err<E>>;

#[derive(Debug, PartialEq)]
pub struct ZFSPoolUsage {
    pub size: u64,
    pub alloc: u64,
    pub free: u64,
    pub dedup: f64,
    pub frag: u64,
}

#[derive(Debug, PartialEq)]
pub struct ZFSPoolStatus {
    pub name: String,
    pub health: String,
    pub usage: Option<ZFSPoolUsage>,
    pub devices: Vec<String>,
}

/// Returns kernel IO-stats for zfs pools
pub fn zfs_pool_stats(pool: &OsStr) -> Result<Option<BlockDevStat>, Error> {

    let mut path = PathBuf::from("/proc/spl/kstat/zfs");
    path.push(pool);
    path.push("io");

    let text = match proxmox::tools::fs::file_read_optional_string(&path)? {
        Some(text) => text,
        None => { return Ok(None); }
    };

    let lines: Vec<&str> = text.lines().collect();

    if lines.len() < 3 {
        bail!("unable to parse {:?} - got less than 3 lines", path);
    }

    // https://github.com/openzfs/zfs/blob/master/lib/libspl/include/sys/kstat.h#L578
    // nread    nwritten reads    writes   wtime    wlentime wupdate  rtime    rlentime rupdate  wcnt     rcnt
    // Note: w -> wait (wtime -> wait time)
    // Note: r -> run  (rtime -> run time)
    // All times are nanoseconds
    let stat: Vec<u64> = lines[2].split_ascii_whitespace().map(|s| {
        u64::from_str_radix(s, 10).unwrap_or(0)
    }).collect();

    let ticks = (stat[4] + stat[7])/1_000_000; // convert to milisec

    let stat = BlockDevStat {
        read_sectors: stat[0]>>9,
        write_sectors: stat[1]>>9,
        read_ios: stat[2],
        write_ios: stat[3],
        io_ticks: ticks,
    };

    Ok(Some(stat))
}

/// Recognizes zero or more spaces and tabs (but not carage returns or line feeds)
fn multispace0(i: &str)  -> IResult<&str, &str> {
    take_while(|c| c == ' ' || c == '\t')(i)
}

/// Recognizes one or more spaces and tabs (but not carage returns or line feeds)
fn multispace1(i: &str)  -> IResult<&str, &str> {
    take_while1(|c| c == ' ' || c == '\t')(i)
}

/// Recognizes one or more non-whitespace-characters
fn notspace1(i: &str)  -> IResult<&str, &str> {
    take_while1(|c| !(c == ' ' || c == '\t' || c == '\n'))(i)
}

fn parse_optional_u64(i: &str) -> IResult<&str, Option<u64>> {
    if i.starts_with('-') {
        Ok((&i[1..], None))
    } else {
        let (i, value) = map_res(recognize(digit1), str::parse)(i)?;
        Ok((i, Some(value)))
    }
}

fn parse_optional_f64(i: &str) -> IResult<&str, Option<f64>> {
    if i.starts_with('-') {
        Ok((&i[1..], None))
    } else {
        let (i, value) = nom::number::complete::double(i)?;
        Ok((i, Some(value)))
    }
}

fn parse_pool_device(i: &str) -> IResult<&str, String> {
    let (i, (device, _, _rest)) = tuple((
        preceded(multispace1, take_till1(|c| c == ' ' || c == '\t')),
        multispace1,
        preceded(take_till(|c| c == '\n'), char('\n')),
    ))(i)?;

    Ok((i, device.to_string()))
}

fn parse_pool_header(i: &str) -> IResult<&str, ZFSPoolStatus> {
    // name, size, allocated, free, checkpoint, expandsize, fragmentation, capacity, dedupratio, health, altroot.

    let (i, (text, size, alloc, free, _, _,
             frag, _, dedup, health,
             _altroot, _eol)) = tuple((
        take_while1(|c| char::is_alphanumeric(c)), // name
        preceded(multispace1, parse_optional_u64), // size
        preceded(multispace1, parse_optional_u64), // allocated
        preceded(multispace1, parse_optional_u64), // free
        preceded(multispace1, notspace1), // checkpoint
        preceded(multispace1, notspace1), // expandsize
        preceded(multispace1, parse_optional_u64), // fragmentation
        preceded(multispace1, notspace1), // capacity
        preceded(multispace1, parse_optional_f64), // dedup
        preceded(multispace1, notspace1), // health
        opt(preceded(multispace1, notspace1)), // optional altroot
        line_ending,
    ))(i)?;

    let status = if let (Some(size), Some(alloc), Some(free), Some(frag), Some(dedup)) = (size, alloc, free, frag, dedup)  {
        ZFSPoolStatus {
            name: text.into(),
            health: health.into(),
            usage: Some(ZFSPoolUsage { size, alloc, free, frag, dedup }),
            devices: Vec::new(),
        }
    } else {
         ZFSPoolStatus {
             name: text.into(),
             health: health.into(),
             usage: None,
             devices: Vec::new(),
         }
    };

    Ok((i, status))
}

fn parse_pool_status(i: &str) -> IResult<&str, ZFSPoolStatus> {

    let (i, mut stat) = parse_pool_header(i)?;
    let (i, devices) = many0(parse_pool_device)(i)?;

    for device_path in devices.into_iter().filter(|n| n.starts_with("/dev/")) {
        stat.devices.push(device_path);
    }

    let (i, _) = many0(tuple((multispace0, char('\n'))))(i)?; // skip empty lines

    Ok((i, stat))
}

/// Parse zpool list outout
///
/// Note: This does not reveal any details on how the pool uses the devices, because
/// the zpool list output format is not really defined...
fn parse_zpool_list(i: &str) -> Result<Vec<ZFSPoolStatus>, Error> {
    match all_consuming(many0(parse_pool_status))(i) {
        Err(nom::Err::Error(err)) |
        Err(nom::Err::Failure(err)) => {
            bail!("unable to parse zfs list output - {}", nom::error::convert_error(i, err));
        }
        Err(err) => {
            bail!("unable to parse calendar event: {}", err);
        }
        Ok((_, ce)) => Ok(ce),
    }
}

/// Run zpool list and return parsed output
///
/// Devices are only included when run with verbose flags
/// set. Without, device lists are empty.
pub fn zpool_list(pool: Option<String>, verbose: bool) -> Result<Vec<ZFSPoolStatus>, Error> {

    // Note: zpools list verbose output can include entries for 'special', 'cache' and 'logs'
    // and maybe other things.

    let mut command = std::process::Command::new("zpool");
    command.args(&["list", "-H", "-p", "-P"]);

    // Note: We do not use -o to define output properties, because zpool command ignores
    // that completely for special vdevs and devices

    if verbose { command.arg("-v"); }

    if let Some(pool) = pool { command.arg(pool); }

    let output = crate::tools::run_command(command, None)?;

    parse_zpool_list(&output)
}

/// Get set of devices used by zfs (or a specific zfs pool)
///
/// The set is indexed by using the unix raw device number (dev_t is u64)
pub fn zfs_devices(
    partition_type_map: &HashMap<String, Vec<String>>,
    pool: Option<String>,
) -> Result<HashSet<u64>, Error> {

    let list = zpool_list(pool, true)?;

    let mut device_set = HashSet::new();
    for entry in list {
        for device in entry.devices {
            let meta = std::fs::metadata(device)?;
            device_set.insert(meta.rdev());
        }
    }

    for device_list in partition_type_map.iter()
        .filter_map(|(uuid, list)| if ZFS_UUIDS.contains(uuid.as_str()) { Some(list) } else { None })
    {
        for device in device_list {
            let meta = std::fs::metadata(device)?;
            device_set.insert(meta.rdev());
        }
    }

    Ok(device_set)
}

#[test]
fn test_zfs_parse_list() -> Result<(), Error> {

    let output = "";

    let data = parse_zpool_list(&output)?;
    let expect = Vec::new();

    assert_eq!(data, expect);

    let output = "btest	427349245952	405504	427348840448	-	-	0	0	1.00	ONLINE	-\n";
    let data = parse_zpool_list(&output)?;
    let expect = vec![
        ZFSPoolStatus {
            name: "btest".to_string(),
            health: "ONLINE".to_string(),
            devices: Vec::new(),
            usage: Some(ZFSPoolUsage {
                size: 427349245952,
                alloc: 405504,
                free: 427348840448,
                dedup: 1.0,
                frag: 0,
            }),
        }];

    assert_eq!(data, expect);

    let output = "\
rpool    535260299264      402852388864      132407910400      -          -          22         75         1.00      ONLINE   -
            /dev/disk/by-id/ata-Crucial_CT500MX200SSD1_154210EB4078-part3    498216206336      392175546368      106040659968      -          -          22         78         -          ONLINE
special                                                                                             -         -         -            -             -         -         -         -   -
            /dev/sda2          37044092928       10676842496       26367250432       -          -          63         28         -          ONLINE
logs                                                                                                 -         -         -            -             -         -         -         -   -
            /dev/sda3          4831838208         1445888 4830392320         -          -          0          0          -          ONLINE

";

    let data = parse_zpool_list(&output)?;
    let expect = vec![
        ZFSPoolStatus {
            name: String::from("rpool"),
            health: String::from("ONLINE"),
            devices: vec![String::from("/dev/disk/by-id/ata-Crucial_CT500MX200SSD1_154210EB4078-part3")],
            usage: Some(ZFSPoolUsage {
                size: 535260299264,
                alloc:402852388864 ,
                free: 132407910400,
                dedup: 1.0,
                frag: 22,
            }),
        },
        ZFSPoolStatus {
            name: String::from("special"),
            health: String::from("-"),
            devices: vec![String::from("/dev/sda2")],
            usage: None,
        },
        ZFSPoolStatus {
            name: String::from("logs"),
            health: String::from("-"),
            devices: vec![String::from("/dev/sda3")],
            usage: None,
        },
    ];

    assert_eq!(data, expect);

    let output = "\
btest	427349245952	761856	427348484096	-	-	0	0	1.00	ONLINE	-
	mirror	213674622976	438272	213674184704	-	-	0	0	-	ONLINE
	/dev/sda1	-	-	-	-	-	-	-	-	ONLINE
	/dev/sda2	-	-	-	-	-	-	-	-	ONLINE
	mirror	213674622976	323584	213674299392	-	-	0	0	-	ONLINE
	/dev/sda3	-	-	-	-	-	-	-	-	ONLINE
	/dev/sda4	-	-	-	-	-	-	-	-	ONLINE
logs               -      -      -        -         -      -      -      -  -
	/dev/sda5	213674622976	0	213674622976	-	-	0	0	-	ONLINE
";

    let data = parse_zpool_list(&output)?;
    let expect = vec![
        ZFSPoolStatus {
            name: String::from("btest"),
            health: String::from("ONLINE"),
            usage: Some(ZFSPoolUsage {
                size: 427349245952,
                alloc: 761856,
                free: 427348484096,
                dedup: 1.0,
                frag: 0,
            }),
            devices: vec![
                String::from("/dev/sda1"),
                String::from("/dev/sda2"),
                String::from("/dev/sda3"),
                String::from("/dev/sda4"),
            ]
        },
        ZFSPoolStatus {
            name: String::from("logs"),
            health: String::from("-"),
            usage: None,
            devices: vec![String::from("/dev/sda5")],
        },
    ];

    assert_eq!(data, expect);

    Ok(())
}
