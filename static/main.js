const CONNECTING = 0
const CONNECTED = 1
const DISCONNECTED = 2
let connectionState = CONNECTING

const protocol = location.protocol === 'http:' ? 'ws' : 'wss'
const socket = new WebSocket(`${protocol}://${location.hostname}:8008`)

socket.addEventListener('error', _ => {
    connectionState = DISCONNECTED
    m.redraw()
})
socket.addEventListener('open', _ => {
    connectionState = CONNECTED
    m.redraw()
})

const send = msg => socket.send(JSON.stringify(msg))

const avatars = [
    '14571e81fa5ce366eb09d6b3cbadea53',
    '2894_Aww_Blob_DarkerEyes',
    '6876_BlobCatLove',
    'ablobspin',
    'amaze',
    'babaleine',
    'bisexual_flag',
    'blobcatlost',
    'blobcatmeltcry',
    'blobcatpeek',
    'blobcatrainbow',
    'blobfingerguns',
    'bloblight',
    'blobraccoon',
    'blobsweat',
    'bon_anniversaire',
    'bonnaum',
    'bzh',
    'camille',
    'ca_pulse',
    'cerbere',
    'chat_jpg',
    'chocolatine',
    'cornichouette',
    'dehornoy',
    'derp',
    'eopeek',
    'excuse',
    'ferrispeek',
    'fogogoh-ananas',
    'fogogoh-grr',
    'fogogoh-peek',
    'fogogoh',
    'grrnoble',
    'grr_peek',
    'grr',
    'grrscute',
    'hhhhaaAAAAHHHH',
    'hyperthink',
    'johanpakontan',
    'JPEG_20191004_194401.jpg',
    'maaaarrkooovicczzzzz',
    'markoccinelle',
    'marko_joy',
    'marko',
    'marko_terminator',
    'matteo',
    'muannob',
    'ohno',
    'oof',
    'pajojo',
    'pa_kontan',
    'pierrotbis',
    'polytech',
    'rustacean-flat-happy-smol',
    'steven_pun',
    'tabouret',
    'thaenkin',
    'theo_sepulcre',
    'thinkin',
    'tinking',
    'transgender_flag',
    'unsafe',
    'vero'
]

// State
let game = ''
let name = ''
let players = []
let question = 'Si tu vois ça c\'est que ça bug, retourne à l\'accueil'
let avatar = avatars[0]
let readyCounter = 0
let answers = []
let answered = false

socket.addEventListener('message', event => {
    const data = JSON.parse(event.data)
    console.log("Received : ", data)
    switch (data.tag) {
        case 'RoomCreated':
            game = data.code
            send({
                tag: 'JoinRoom',
                username: document.getElementById('username').value,
                avatar: avatar,
                code: game
            })
            break
        case 'OnRoomJoin':
            players = data.players
            m.route.set('/lobby')
            break
        case 'RoomUpdate':
            players = data.players
            m.redraw()
            break
        case 'NewRound':
            question = data.question
            answered = false
            readyCounter = 0
            m.route.set('/question')
            break
        case 'RoundUpdate':
            readyCounter = data.ready_player_count
            m.redraw()
            break
        case 'RoundOver':
            answers = []
            for (const vote of data.votes) {
                let idx = answers.findIndex(x => x.username == vote[1])
                if (idx == -1) {
                    answers.push({
                        username: vote[1],
                        avatar: players.find(x => x.username == vote[1]).avatar,
                        votes: 0
                    })
                    idx = answers.length - 1
                }
                answers[idx].votes += 1
            }
            answers.sort((a, b) => a.votes - b.votes)
            m.route.set('/result')
            break
        case 'GameOver':
            m.route('/home')
        default:
            console.log('Unknown message', data)
            break
    }
})

const showConnectionState = () => {
    if (connectionState == DISCONNECTED) {
        return m('div', { className: 'warning' }, 'T\'es hors ligne')
    } else if (connectionState == CONNECTING) {
        return m('div', { className: 'info' }, 'Connection en cours…')
    } else {
        return null
    }
}

const Home = {
    view: () => {
        return m('main', {}, [
            showConnectionState(),
            m('section', {}, [
                m('h2', {}, 'T ki'),
                m('p', {}, 'Entre ton nom et choisis ton avatar.'),
                m('input', { id: 'username' }),
                m('div', { id: 'avatar-selector' }, avatars.map(a =>
                    m('img', {
                        id: a,
                        className: a == avatar ? 'selected' : '',
                        src: `/static/avatars/${a}.png`,
                        onclick: _ => {
                            document.getElementById(avatar).classList.remove('selected')
                            avatar = a
                            document.getElementById(avatar).classList.add('selected')
                        }
                    })  
                ))
            ]),
            m('section', {}, [
                m('h2', {}, 'Créer une partie'),
                m('p', {}, 'Creéz une partie depuis un ordinateur (ou autre grand écran que tout le monde peut voir). Vous pourrez ensuite la rejoindre avec vos téléphones.'),
                m('a', { className: 'button', onclick: () => {
                    name = document.getElementById('username').value
                    send({
                        tag: 'CreateRoom'
                    })
                } }, 'Créer une partie')
            ]),
            m('section', {}, [
                m('h2', {}, 'Rejoindre une partie'),
                m('p', {}, 'Entrez le code de la partie qui s\'affiche sur le grand écran.'),
                m('input', { id: 'code' }),
                m('a', { className: 'button', href: '#', onclick: () => {
                    game = document.getElementById('code').value
                    name = document.getElementById('username').value
                    send({
                        tag: 'JoinRoom',
                        avatar: avatar,
                        username: name,
                        code: game
                    })
                } }, 'Rejoindre')
            ])
        ])
    }
}

const LobbyLGBT = {
    view: () => m('main', {}, [
        showConnectionState(),
        m('h2', {}, `Code de la partie : ${game}`),
        m('section', { className: 'lobby' },
            players.map(x => m('div', {}, [
                m('img', { className: 'avatar', src: `/static/avatars/${x.avatar}.png` }),
                m('p', {}, x.username),
            ]))
        ),
        name == players[0].username ? m('a', { className: 'button', href: '#', onclick: () => {
            send({
                tag: 'StartRound',
                code: game
            })
        } }, 'Lancer la partie') : null
    ])
}

const Question = {
    view: () => m('main', {}, [
        showConnectionState(),
        m('h2', {}, question),
        m('h3', {}, `${readyCounter} / ${players.length} réponses`),
        m('section', { className: 'choices' }, players.map(x => m('a', { className: `button ${answered ? 'disabled' : ''}`, onclick: () => {
            send({
                tag: 'Answer',
                code: game,
                vote: [name, x.username]
            })
            answered = true
        } }, [
            m('img', { className: 'avatar', src: `/static/avatars/${x.avatar}.png` }),
            m('p', {}, x.username)
        ])))
    ])
}

const Result = {
    view: () => m('main', {}, [
        showConnectionState(),
        m('h2', {}, 'Résultats'),
        m('h3', {}, question),
        m('section', {}, answers.map((a, i) =>
            m('div', { className: 'result', style: `--percent: ${a.votes / players.length * 100}%; animation-delay: ${i * 0.2}s;` }, [
                m('img', { className: 'avatar', src: `/static/avatars/${a.avatar}.png` }),
                m('p', {}, a.username),
                m('p', {}, `${a.votes} vote${a.votes <= 1 ? '' : 's'}`)
            ])
        )),
        name == players[0].username ? m('a', { className: 'button', href: '#', onclick: () => {
            send({
                tag: 'StartRound',
                code: game
            })
        } }, 'Question suivante') : null
    ])
}

m.route(document.getElementById('app'), '/home', {
    '/home': Home,
    '/lobby': LobbyLGBT,
    '/question': Question,
    '/result': Result,
})
