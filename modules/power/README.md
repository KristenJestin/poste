# poste power

Alertes batterie et disque par un petit démon utilisateur (`poste-power watch`, lancé par l'unité
systemd `poste-power.service` que `poste sys setup` active). Il remplace les scripts waybar faits à
la hâte de l'ancien portable pro (`waybar-battery-ac-warn`, `waybar-disk-warn`,
`omarchy-battery-monitor-plus` et son drop-in systemd). Prévu pour le portable ; sur une machine sans
batterie il ne fait que l'alerte disque (voir `disabled` plus bas pour ne pas l'installer).

## Alertes

Chacune part UNE fois par passage de seuil, et se réarme quand la condition disparaît. Notification par
`omarchy-notification-send` (style de l'île, application « Énergie »).

| Alerte | Condition | Urgence | Réarmement |
| --- | --- | --- | --- |
| Batterie faible | niveau <= 20 % et la batterie ne charge pas | normale | on charge, ou niveau > 20 % |
| Batterie critique | niveau <= 10 % et la batterie ne charge pas | urgente | on charge, ou niveau > 10 % |
| Branché, mais la batterie se vide | secteur branché ET (statut « Discharging » OU perte réelle > 12 W) pendant 2 passes de suite | urgente | la perte cesse |
| Disque presque plein | occupation >= 90 % sur `/` et `/home` (ignoré si `/home` est sur le même disque que `/`) | normale | occupation < seuil - 2 points (hystérésis) |

- « Ne charge pas » = statut différent de `Charging` et `Full` (`Not charging` compte : le chargeur
  USB-C d'un portable peut ne pas suivre).
- La perte réelle se mesure sur `energy_now` (ou `charge_now` x `voltage_now`) entre deux passes : le
  statut et `power_now` mentent parfois (leçon de l'ancien portable, chargeur USB-C instable). Une
  mesure après une longue veille est ignorée.
- Un saut direct sous 10 % ne déclenche que « critique ». Une alerte dont l'envoi échoue est retentée à la
  passe suivante.
- Seules les alimentations du système comptent (`scope` différent de `Device` : une pile de souris
  `hidpp_battery_*` n'est jamais « la batterie »). Secteur = type `Mains` en ligne.
- Sans batterie : seule l'alerte disque.
- Pas d'alerte de branchement / débranchement : les références n'en avaient pas (le bruit ne sert à rien).

## Commandes

- `poste power status` : batterie, secteur, disques, seuils, alertes déjà envoyées.
- `poste power test` : envoie une notification d'exemple.
- `poste power check` : une passe (ce que le démon fait toutes les minutes).
- `poste power watch` : le démon.

## Réglages : `~/.config/poste/power.toml`

Facultatif ; chaque clé absente garde sa valeur par défaut. Relu à chaque passe (pas de redémarrage).

```toml
interval = 60            # secondes entre deux passes (10 au minimum)

[battery]
low = 20
critical = 10
drain_w = 12.0           # perte (W) qui compte comme « se vide sur secteur »
drain_samples = 2        # passes consécutives avant d'alerter

[disk]
percent = 90
free_gb = 0              # alerter aussi sous ce nombre de Go libres (0 : non)
paths = ["/", "/home"]
```

## Démon léger

Une passe toutes les 60 s : quelques lectures de `/sys/class/power_supply/*` et un `statvfs` par
disque, puis `sleep`. Pas d'udev (le sondage de 60 s suffit et reste simple). L'état « déjà alerté »
est dans `$XDG_RUNTIME_DIR/poste-power/state.json` (perdu au redémarrage de la machine, conservé quand le
démon redémarre). L'unité (`systemd/poste-power.service`) tourne en `Nice=19` avec `MemoryMax=64M`.

## Ne pas l'installer sur une machine

Dans `~/.config/poste/config.toml` de cette machine : `poste sys module disable power` (le prochain
`poste sys setup` retire alors fichiers et unité, avec aperçu).

## Développement

Variables RÉSERVÉES AUX TESTS : `POSTE_POWER_SUPPLY_DIR` (faux `power_supply` en dossier temporaire) et
`POSTE_POWER_NOTIFY` (remplace `omarchy-notification-send`). Les tests (`cargo test -p poste-power`) :
seuils et réarmement (`src/engine.rs`), lecture d'un faux arbre (`src/supply.rs`), passes successives
(`src/lib.rs`), binaire complet avec fausses notifications (`tests/cli.rs`).
