# Repairing a file that an older Jack tagged wrongly

Jack has identified only since #25. He runs `exiftool -j`, `ffprobe` and `yt-dlp` to read, and
nothing that writes. A test (`jack_has_no_way_to_write_to_a_media_file`) fails the build if a
tag writer comes back into his code. Builds from before #25 also tagged files, with AtomicParsley
or exiftool, overwriting in place. If one of those builds tagged a file it had marked unresolved,
the file now holds another work's metadata. These steps undo that. `<video.mp4>` stands for the
damaged file.

## 1. Make sure the current Jack is the one that runs

```sh
which -a jack          # every jack on PATH; the first one runs
jack -h -j             # only a current build has `-h -j`; an old build says unknown flag
```

`jack setup` leaves an existing `~/.local/bin/jack` alone if it is not Jack's own link ("left
... alone (not ours)"). If that file is an older build, it still runs first. Run the new build
by its full path, or replace that file with a link to the new build yourself.

## 2. Is there a pre-write copy?

The old writer used `--overWrite` / `-overwrite_original` and kept no copy of the file. Jack's
`backups/` folder holds copies of `jack.db` only, not of media files. So Jack cannot restore the
file's old tags. If a copy exists elsewhere (another drive, a Time Machine backup or the original
download), copying it back is the cleanest fix, and you can stop here.

## 3. Clear the wrong tags

```sh
cp <video.mp4> <video.mp4>.bak                   # keep a copy until you are happy
AtomicParsley <video.mp4> -t                     # what is there now (iTunes-style atoms)
AtomicParsley <video.mp4> --metaEnema --overWrite
exiftool -a -G1 -s <video.mp4>                   # check nothing wrong is left
```

`--metaEnema` removes every iTunes-style atom, including the cover art and the custom
`com.apple.iTunes` id atoms (`IMDB_ID`, `TMDB_ID`, `TVDB_ID`, ratings) the old tagger wrote. If
`exiftool` still shows a wrong value in another group, clear that group:
`exiftool -overwrite_original -UserData:all= -XMP:all= <video.mp4>`.

For an MKV file, the old tagger used `mkvpropedit`. Run `mkvpropedit <video.mkv> --tags all:`
(clears all tags) and `mkvpropedit <video.mkv> --delete-attachment mime-type:image/jpeg`
(cover art).

## 4. Re-tag from the right identity

First identify the file with the current Jack and an id. Use `-j` for JSON:

```sh
jack <video.mp4> --thetvdb <id> --year <year> -j
```

Tagging is Lauren's job (the metadata-writing windup), and she isn't built yet. Until she is, write
the tags yourself from Jack's answer, and only if Jack reported the file as identified:

```sh
AtomicParsley <video.mp4> --overWrite --stik Movie \
  --title "<title>" --year "<year>" --description "<description>" \
  --rDNSatom "<imdb id>" name=IMDB_ID domain=com.apple.iTunes \
  --rDNSatom "<tmdb id>" name=TMDB_ID domain=com.apple.iTunes \
  --rDNSatom "<thetvdb id>" name=TVDB_ID domain=com.apple.iTunes
```

Example ground truth (the Nov 27 regression): Mountain (2017 documentary), TheTVDB movies id
27498, TMDB 462145, IMDb tt6203570. The wrong values to look for: Brokeback Mountain, IMDb
tt0388795, TMDB 142.
